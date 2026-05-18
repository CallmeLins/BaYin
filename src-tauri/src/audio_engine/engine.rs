use crossbeam_channel::{Receiver, Sender};
use ringbuf::HeapProd;
use ringbuf::traits::{Observer, Producer};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
#[cfg(target_os = "android")]
use tauri::Manager;

use super::decoder::AudioDecoder;
use super::dsp::Equalizer;
use super::fft::FftProcessor;
use super::output::AudioOutput;
use super::resampler::AudioResampler;
#[cfg(target_os = "android")]
use crate::audio_engine::AudioEngineState;
#[cfg(target_os = "android")]
use crate::playback::PlaybackDomainState;
#[cfg(target_os = "android")]
use crate::playback_control;

const FADE_OUT_MS: f32 = 150.0;
const FADE_IN_MS: f32 = 50.0;  // 减少淡入时间到 50ms，加快播放响应
const PENDING_SAMPLES_MAX: usize = 1_048_576;
const CMD_CHANNEL_CAP: usize = 256;

enum FadeAction {
    Pause,
    Stop,
    PlayNext { source: String },
}

enum FadeState {
    None,
    FadingIn { gain: f32, step: f32 },
    FadingOut { gain: f32, step: f32, action: FadeAction },
}

pub enum AudioCommand {
    Play { source: String },
    Pause,
    Resume,
    Stop,
    Seek { position_secs: f64 },
    #[cfg(target_os = "android")]
    RefreshOutput,
    SetVolume { volume: f32 },
    SetEqBands { gains: [f32; 10] },
    SetEqEnabled { enabled: bool },
    EnableVisualization { enabled: bool },
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 1.0,
        }
    }
}

#[derive(Clone, Serialize)]
struct TimePayload {
    position: f64,
    duration: f64,
    session_id: u64,
}

#[derive(Clone, Serialize)]
struct FftPayload {
    frequency: Vec<u8>,
    waveform: Vec<u8>,
}

#[derive(Clone, Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Clone, Serialize)]
struct StateChangedPayload {
    is_playing: bool,
}

pub struct AudioEngine {
    cmd_tx: Sender<AudioCommand>,
    pub state: Arc<Mutex<PlaybackState>>,
}

impl AudioEngine {
    pub fn new(app_handle: AppHandle) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::bounded(CMD_CHANNEL_CAP);
        let state = Arc::new(Mutex::new(PlaybackState {
            is_playing: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 1.0,
        }));
        let state_clone = state.clone();

        std::thread::Builder::new()
            .name("audio-engine".into())
            .spawn(move || {
                audio_thread(cmd_rx, state_clone, app_handle);
            })
            .expect("Failed to spawn audio engine thread");

        Self { cmd_tx, state }
    }

    pub fn send(&self, cmd: AudioCommand) {
        if let Err(err) = self.cmd_tx.try_send(cmd) {
            log::warn!(
                "Audio command channel full, dropping command: {:?}",
                std::mem::discriminant(&err.into_inner())
            );
        }
    }
}

// ── AudioThreadState ────────────────────────────────────────────

struct AudioThreadState {
    decoder: Option<AudioDecoder>,
    output: Option<AudioOutput>,
    eq: Equalizer,
    fft_proc: FftProcessor,
    resampler: Option<AudioResampler>,
    resample_buffer: Vec<f32>,
    pending_samples: Vec<f32>,

    volume: f32,
    position_secs: f64,
    duration_secs: f64,
    is_playing: bool,
    source_sample_rate: u32,
    source_channels: usize,
    fade_state: FadeState,
    end_pending: bool,
    pause_target_secs: Option<f64>,

    last_time_emit: Instant,
    last_fft_emit: Instant,
    session_id: u64,
    last_emitted_pos: f64,

    // Device error recovery cooldown
    last_output_error: Option<Instant>,
    output_error_count: u32,
}

impl AudioThreadState {
    fn new() -> Self {
        Self {
            decoder: None,
            output: None,
            eq: Equalizer::new(44100, 2),
            fft_proc: FftProcessor::new(),
            resampler: None,
            resample_buffer: Vec::new(),
            pending_samples: Vec::new(),

            volume: 1.0,
            position_secs: 0.0,
            duration_secs: 0.0,
            is_playing: false,
            source_sample_rate: 44100,
            source_channels: 2,
            fade_state: FadeState::None,
            end_pending: false,
            pause_target_secs: None,

            last_time_emit: Instant::now(),
            last_fft_emit: Instant::now(),
            session_id: 0,
            last_emitted_pos: 0.0,

            last_output_error: None,
            output_error_count: 0,
        }
    }

    // ── helpers ──

    fn out_rate(&self) -> u32 {
        self.output
            .as_ref()
            .map(|o| o.config.sample_rate.0)
            .unwrap_or(self.source_sample_rate)
    }

    fn out_ch(&self) -> usize {
        self.output
            .as_ref()
            .map(|o| o.config.channels as usize)
            .unwrap_or(2)
    }

    fn playback_position(&self) -> f64 {
        if let Some(out) = self.output.as_ref() {
            let samples = out.producer.occupied_len();
            let rate = out.config.sample_rate.0 as f64;
            let ch = out.config.channels as f64;
            if rate > 0.0 && ch > 0.0 {
                let secs = samples as f64 / (rate * ch);
                return (self.position_secs - secs).max(0.0);
            }
        }
        self.position_secs.max(0.0)
    }

    fn bump_session(&mut self) {
        self.session_id = self.session_id.wrapping_add(1);
        if self.session_id == 0 {
            self.session_id = 1;
        }
    }

    fn current_gain(&self) -> f32 {
        match &self.fade_state {
            FadeState::FadingIn { gain, .. } => *gain,
            FadeState::FadingOut { gain, .. } => *gain,
            FadeState::None => 1.0,
        }
    }

    fn reset_output_state(&mut self) {
        self.output = None;
        self.resampler = None;
        self.resample_buffer.clear();
        self.pending_samples.clear();
        self.fade_state = FadeState::None;
    }

    fn emit_state_changed(&self, app_handle: &AppHandle, playing: bool) {
        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: playing });
    }

    // ── core operations ──

    /// Open a new source and set up output. Returns true on success.
    fn execute_play(
        &mut self,
        source: &str,
        with_fade_in: bool,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) -> bool {
        self.decoder = None;
        self.output = None;
        self.resampler = None;
        self.resample_buffer.clear();
        self.is_playing = false;
        self.position_secs = 0.0;

        let dec = match AudioDecoder::open(source) {
            Ok(d) => d,
            Err(e) => {
                let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                return false;
            }
        };

        self.source_sample_rate = dec.info.sample_rate;
        self.source_channels = dec.info.channels;
        self.duration_secs = dec.info.duration_secs;

        let output_channels = (self.source_channels).min(2) as u16;

        let out = match AudioOutput::new(self.source_sample_rate, output_channels) {
            Ok(o) => o,
            Err(e) => {
                let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                return false;
            }
        };

        let out_rate = out.config.sample_rate.0;
        let actual_channels = out.config.channels as usize;

        if out_rate != self.source_sample_rate {
            match AudioResampler::new(self.source_sample_rate, out_rate, actual_channels) {
                Ok(rs) => self.resampler = Some(rs),
                Err(e) => log::warn!("Resampler init warning: {}", e),
            }
        }

        let effective_rate = if self.resampler.is_some() { out_rate } else { self.source_sample_rate };
        {
            let mut new_eq = Equalizer::new(effective_rate, actual_channels);
            new_eq.set_enabled(self.eq.is_enabled());
            std::mem::swap(&mut self.eq, &mut new_eq);
        }

        let fade_rate = if self.resampler.is_some() { out_rate } else { self.source_sample_rate };
        let fade_ch = actual_channels;

        self.output = Some(out);
        self.decoder = Some(dec);
        self.is_playing = true;

        if with_fade_in {
            self.fade_state = FadeState::FadingIn {
                gain: 0.0,
                step: fade_step(FADE_IN_MS, fade_rate, fade_ch),
            };
        } else {
            self.fade_state = FadeState::None;
        }

        update_state(state, true, self.position_secs, self.duration_secs, self.volume);
        self.emit_state_changed(app_handle, true);
        true
    }

    fn reinitialize_output_for_route_change(
        &mut self,
        should_play: bool,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        let dec = self.decoder.as_mut().ok_or("no decoder")?;
        let target = {
            if let Some(out) = self.output.as_ref() {
                let samples = out.producer.occupied_len();
                let rate = out.config.sample_rate.0 as f64;
                let ch = out.config.channels as f64;
                if rate > 0.0 && ch > 0.0 {
                    (self.position_secs - samples as f64 / (rate * ch)).max(0.0)
                } else {
                    self.position_secs
                }
            } else {
                self.position_secs
            }
        };
        let output_channels = self.source_channels.min(2) as u16;

        let new_output = AudioOutput::new(self.source_sample_rate, output_channels)?;
        let out_rate = new_output.config.sample_rate.0;
        let actual_channels = new_output.config.channels as usize;

        dec.seek(target)
            .map_err(|e| format!("Failed to seek during output reinit: {e}"))?;

        self.output = Some(new_output);
        if !should_play {
            if let Some(ref out) = self.output {
                out.pause();
            }
        }

        self.resampler = None;
        self.resample_buffer.clear();
        self.pending_samples.clear();

        if out_rate != self.source_sample_rate {
            match AudioResampler::new(self.source_sample_rate, out_rate, actual_channels) {
                Ok(rs) => self.resampler = Some(rs),
                Err(e) => log::warn!("Resampler reinit warning after route change: {}", e),
            }
        }

        let effective_rate = if self.resampler.is_some() { out_rate } else { self.source_sample_rate };
        let eq_enabled = self.eq.is_enabled();
        let eq_gains = self.eq.gains();
        let mut new_eq = Equalizer::new(effective_rate, actual_channels);
        new_eq.set_enabled(eq_enabled);
        new_eq.set_gains(&eq_gains);
        self.eq = new_eq;

        self.position_secs = target;
        self.last_emitted_pos = target;
        self.fade_state = if should_play {
            FadeState::FadingIn {
                gain: 0.0,
                step: fade_step(FADE_IN_MS, effective_rate, actual_channels),
            }
        } else {
            FadeState::None
        };

        update_state(state, should_play, self.position_secs, self.duration_secs, self.volume);
        self.emit_state_changed(app_handle, should_play);
        Ok(())
    }

    // ── main-loop sections ──

    /// Auto-recover output stream failures (route change / spatial effect toggle).
    fn handle_output_error(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        let failed = self.output.as_ref().map(|o| o.has_stream_error()).unwrap_or(false);
        if !failed {
            self.output_error_count = 0;
            return;
        }

        // Cooldown: don't retry more than once per 2 seconds
        let now = Instant::now();
        if let Some(last_err) = self.last_output_error {
            if now.duration_since(last_err) < Duration::from_secs(2) {
                return; // Still in cooldown
            }
        }
        self.last_output_error = Some(now);
        self.output_error_count += 1;

        self.pause_target_secs = None;
        self.end_pending = false;

        // After 5 consecutive failures, give up and stop playback
        if self.output_error_count > 5 {
            log::warn!("Audio output: {} consecutive device errors, stopping playback", self.output_error_count);
            self.is_playing = false;
            self.reset_output_state();
            update_state(state, false, self.position_secs, self.duration_secs, self.volume);
            let _ = app_handle.emit(
                "audio:error",
                ErrorPayload {
                    message: "Audio device disconnected and recovery failed".to_string(),
                },
            );
            self.emit_state_changed(app_handle, false);
            return;
        }

        if self.decoder.is_some() {
            let should_play = self.is_playing;
            if let Err(e) = self.reinitialize_output_for_route_change(should_play, state, app_handle) {
                log::warn!("Audio output recovery attempt {} failed: {}", self.output_error_count, e);
                // Don't stop immediately - the next loop iteration will retry after cooldown
            }
        } else {
            self.is_playing = false;
            self.reset_output_state();
            update_state(state, false, self.position_secs, self.duration_secs, self.volume);
            self.emit_state_changed(app_handle, false);
        }
    }

    /// Process pending commands. Returns `true` if `continue` needed for the main loop.
    fn handle_cmds(
        &mut self,
        cmd_rx: &Receiver<AudioCommand>,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCommand::Play { source } => self.handle_play(&source, state, app_handle),
                AudioCommand::Pause => self.handle_pause(state, app_handle),
                AudioCommand::Resume => self.handle_resume(state, app_handle),
                AudioCommand::Stop => self.handle_stop(state, app_handle),
                AudioCommand::Seek { position_secs } => self.handle_seek(position_secs, state),
                #[cfg(target_os = "android")]
                AudioCommand::RefreshOutput => self.handle_refresh_output(state, app_handle),
                AudioCommand::SetVolume { volume } => {
                    self.volume = volume.clamp(0.0, 1.0);
                    update_state(
                        state,
                        self.is_playing,
                        self.position_secs,
                        self.duration_secs,
                        self.volume,
                    );
                }
                AudioCommand::SetEqBands { gains } => self.eq.set_gains(&gains),
                AudioCommand::SetEqEnabled { enabled } => self.eq.set_enabled(enabled),
                AudioCommand::EnableVisualization { enabled } => self.fft_proc.set_enabled(enabled),
            }
        }
    }

    fn handle_play(
        &mut self,
        source: &str,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        self.pause_target_secs = None;
        let was_draining = self.end_pending;
        self.end_pending = false;

        if self.is_playing {
            if was_draining {
                if let Some(ref out) = self.output {
                    out.flush();
                }
                self.pending_samples.clear();
                self.bump_session();
                if self.execute_play(source, true, state, app_handle) {
                    self.last_emitted_pos = 0.0;
                }
                self.pending_samples.clear();
                return;
            }
            // Currently playing: fade out then switch
            if let Some(ref out) = self.output {
                out.flush();
            }
            self.fade_state = FadeState::FadingOut {
                gain: self.current_gain(),
                step: fade_step(FADE_OUT_MS, self.out_rate(), self.out_ch()),
                action: FadeAction::PlayNext {
                    source: source.to_string(),
                },
            };
        } else {
            self.bump_session();
            if self.execute_play(source, true, state, app_handle) {
                self.last_emitted_pos = 0.0;
            }
            self.pending_samples.clear();
        }
    }

    fn handle_pause(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        if !self.is_playing {
            return;
        }

        self.pause_target_secs = Some(self.playback_position());

        if self.end_pending {
            self.is_playing = false;
            if let Some(ref out) = self.output {
                out.pause();
            }
            if let Some(target) = self.pause_target_secs.take() {
                self.position_secs = target;
                self.last_emitted_pos = target;
            }
            update_state(state, false, self.position_secs, self.duration_secs, self.volume);
            self.emit_state_changed(app_handle, false);
            return;
        }

        self.fade_state = FadeState::FadingOut {
            gain: self.current_gain(),
            step: fade_step(FADE_OUT_MS, self.out_rate(), self.out_ch()),
            action: FadeAction::Pause,
        };
    }

    fn handle_resume(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        if !self.is_playing && self.decoder.is_some() {
            self.pause_target_secs = None;
            self.is_playing = true;
            if let Some(ref out) = self.output {
                out.resume();
            }
            self.fade_state = FadeState::FadingIn {
                gain: 0.0,
                step: fade_step(FADE_IN_MS, self.out_rate(), self.out_ch()),
            };
            update_state(state, true, self.position_secs, self.duration_secs, self.volume);
            self.emit_state_changed(app_handle, true);
        } else if self.is_playing {
            if let FadeState::FadingOut { gain, action: FadeAction::Pause, .. } = &self.fade_state {
                let current_gain = *gain;
                self.pause_target_secs = None;
                self.fade_state = FadeState::FadingIn {
                    gain: current_gain,
                    step: fade_step(FADE_IN_MS, self.out_rate(), self.out_ch()),
                };
            }
        }
    }

    fn handle_stop(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        self.pause_target_secs = None;
        let was_draining = self.end_pending;
        self.end_pending = false;

        if self.is_playing {
            if was_draining {
                self.decoder = None;
                self.output = None;
                self.resampler = None;
                self.resample_buffer.clear();
                self.pending_samples.clear();
                self.is_playing = false;
                self.position_secs = 0.0;
                self.duration_secs = 0.0;
                self.fade_state = FadeState::None;
                self.fft_proc.set_enabled(false);
                update_state(state, false, 0.0, 0.0, self.volume);
                self.emit_state_changed(app_handle, false);
                return;
            }
            if let Some(ref out) = self.output {
                out.flush();
            }
            self.fade_state = FadeState::FadingOut {
                gain: self.current_gain(),
                step: fade_step(FADE_OUT_MS, self.out_rate(), self.out_ch()),
                action: FadeAction::Stop,
            };
        } else {
            self.decoder = None;
            self.output = None;
            self.resampler = None;
            self.resample_buffer.clear();
            self.pending_samples.clear();
            self.position_secs = 0.0;
            self.duration_secs = 0.0;
            self.fade_state = FadeState::None;
            self.fft_proc.set_enabled(false);
            update_state(state, false, 0.0, 0.0, self.volume);
            self.emit_state_changed(app_handle, false);
        }
    }

    fn handle_seek(
        &mut self,
        pos: f64,
        state: &Arc<Mutex<PlaybackState>>,
    ) {
        self.pause_target_secs = None;
        self.end_pending = false;

        if let Some(ref mut dec) = self.decoder {
            let clamped = if self.duration_secs > 0.0 {
                pos.clamp(0.0, self.duration_secs)
            } else {
                pos.max(0.0)
            };
            if let Err(e) = dec.seek(clamped) {
                log::warn!("Seek error: {}", e);
            } else {
                self.position_secs = clamped;
                self.last_emitted_pos = clamped;
                if let Some(ref out) = self.output {
                    out.flush();
                }
                self.eq.reset();
                self.pending_samples.clear();
                update_state(
                    state,
                    self.is_playing,
                    self.position_secs,
                    self.duration_secs,
                    self.volume,
                );
            }
        }
    }

    #[cfg(target_os = "android")]
    fn handle_refresh_output(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        self.pause_target_secs = None;
        self.end_pending = false;

        if self.decoder.is_none() {
            return;
        }

        let should_play = self.is_playing;
        if let Err(e) = self.reinitialize_output_for_route_change(should_play, state, app_handle) {
            let _ = app_handle.emit(
                "audio:error",
                ErrorPayload {
                    message: format!(
                        "Failed to reinitialize audio output after route change: {}",
                        e
                    ),
                },
            );
        }
    }

    /// Decode and feed output. Returns true when a fade-out completed.
    fn decode_and_feed(&mut self, app_handle: &AppHandle) -> bool {
        if !self.is_playing {
            return false;
        }

        let (mut fade_completed, out_channels) = match (&mut self.decoder, &mut self.output) {
            (Some(_dec), Some(out)) => (false, out.config.channels as usize),
            _ => return false,
        };

        // Flush pending samples first
        if let Some(ref mut out) = self.output {
            if !self.pending_samples.is_empty() {
                let written = out.producer.push_slice(&self.pending_samples);
                if written > 0 {
                    self.pending_samples.drain(..written);
                }
            }
        }

        for _ in 0..32 {
            if self.end_pending || !self.pending_samples.is_empty() {
                break;
            }

            let available = match &self.output {
                Some(out) => out.producer.vacant_len(),
                None => break,
            };
            if available < 8192 {
                break;
            }

            let dec = match &mut self.decoder {
                Some(d) => d,
                None => break,
            };

            match dec.decode_next() {
                Ok(Some(mut samples)) => {
                    let decoded_channels = self.source_channels;
                    let decoded_frames = samples.len() / decoded_channels;

                    if decoded_channels != out_channels {
                        samples = convert_channels(&samples, decoded_channels, out_channels);
                    }

                    if let Some(ref mut _rs) = self.resampler {
                        fade_completed = self
                            .process_resampler(&samples, out_channels)
                            || fade_completed;
                    } else {
                        self.eq.process(&mut samples);
                        self.fft_proc.push_samples(&samples, out_channels);
                        if apply_volume_with_fade(&mut samples, self.volume, &mut self.fade_state) {
                            if let Some(ref mut out) = self.output {
                                push_or_pend(&mut out.producer, &samples, &mut self.pending_samples);
                            }
                            fade_completed = true;
                        }
                        if !fade_completed {
                            if let Some(ref mut out) = self.output {
                                push_or_pend(&mut out.producer, &samples, &mut self.pending_samples);
                            }
                        }
                    }

                    if fade_completed {
                        return true;
                    }

                    self.position_secs += decoded_frames as f64 / self.source_sample_rate as f64;
                    if self.position_secs > self.duration_secs && self.duration_secs > 0.0 {
                        self.position_secs = self.duration_secs;
                    }
                }
                Ok(None) => {
                    if self.duration_secs <= 0.0
                        || (self.position_secs - self.duration_secs).abs() > 1.0
                    {
                        self.duration_secs = self.position_secs;
                    }
                    self.end_pending = true;
                    return false;
                }
                Err(e) => {
                    self.is_playing = false;
                    self.fade_state = FadeState::None;
                    let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                    return false;
                }
            }
        }

        false
    }

    /// Process resampler pipeline for one decoded chunk.
    fn process_resampler(&mut self, samples: &[f32], out_channels: usize) -> bool {
        let rs = match &mut self.resampler {
            Some(r) => r,
            None => return false,
        };
        let out = match &mut self.output {
            Some(o) => o,
            None => return false,
        };

        self.resample_buffer.extend_from_slice(samples);
        let needed = rs.input_frames_needed() * out_channels;

        while self.resample_buffer.len() >= needed && self.pending_samples.is_empty() {
            let chunk: Vec<f32> = self.resample_buffer.drain(..needed).collect();
            match rs.process(&chunk) {
                Ok(mut resampled) => {
                    self.eq.process(&mut resampled);
                    self.fft_proc.push_samples(&resampled, out_channels);
                    if apply_volume_with_fade(&mut resampled, self.volume, &mut self.fade_state) {
                        push_or_pend(&mut out.producer, &resampled, &mut self.pending_samples);
                        return true;
                    }
                    push_or_pend(&mut out.producer, &resampled, &mut self.pending_samples);
                }
                Err(e) => log::warn!("Resample error: {}", e),
            }
            let next_needed = rs.input_frames_needed() * out_channels;
            if self.resample_buffer.len() < next_needed {
                break;
            }
        }
        false
    }

    /// Handle fade-out completion.
    fn handle_fade_completion(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        let action = std::mem::replace(&mut self.fade_state, FadeState::None);
        let fade_action = match action {
            FadeState::FadingOut { action, .. } => action,
            _ => return,
        };

        match fade_action {
            FadeAction::Pause => {
                self.is_playing = false;
                if let Some(ref out) = self.output {
                    out.pause();
                }
                if let Some(target) = self.pause_target_secs.take() {
                    if let Some(ref mut dec) = self.decoder {
                        if let Err(e) = dec.seek(target) {
                            log::warn!("Pause seek-back error: {}", e);
                        } else {
                            self.position_secs = target;
                            self.last_emitted_pos = target;
                            if let Some(ref out) = self.output {
                                out.flush();
                            }
                            self.eq.reset();
                            self.pending_samples.clear();
                        }
                    } else {
                        self.position_secs = target;
                        self.last_emitted_pos = target;
                    }
                }
                update_state(
                    state,
                    false,
                    self.position_secs,
                    self.duration_secs,
                    self.volume,
                );
                self.emit_state_changed(app_handle, false);
            }
            FadeAction::Stop => {
                self.pause_target_secs = None;
                self.decoder = None;
                self.output = None;
                self.resampler = None;
                self.resample_buffer.clear();
                self.pending_samples.clear();
                self.is_playing = false;
                self.position_secs = 0.0;
                self.duration_secs = 0.0;
                self.last_emitted_pos = 0.0;
                self.fade_state = FadeState::None;
                self.fft_proc.set_enabled(false);
                update_state(state, false, 0.0, 0.0, self.volume);
                self.emit_state_changed(app_handle, false);
            }
            FadeAction::PlayNext { source } => {
                self.pause_target_secs = None;
                self.bump_session();
                if self.execute_play(&source, true, state, app_handle) {
                    self.last_emitted_pos = 0.0;
                }
                self.pending_samples.clear();
            }
        }
    }

    /// Natural end: emit `audio:ended` once the output ring buffer drains.
    fn handle_natural_end(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        if !self.end_pending {
            return;
        }

        let buffered = self
            .output
            .as_ref()
            .map(|o| o.producer.occupied_len())
            .unwrap_or(0);

        if !self.pending_samples.is_empty() || buffered != 0 {
            return;
        }

        #[cfg(target_os = "android")]
        {
            if self.try_android_auto_advance(state, app_handle) {
                self.pending_samples.clear();
                return;
            }
        }

        self.end_pending = false;
        self.is_playing = false;
        self.pause_target_secs = None;
        self.fade_state = FadeState::None;
        update_state(
            state,
            false,
            self.duration_secs,
            self.duration_secs,
            self.volume,
        );
        let _ = app_handle.emit("audio:ended", ());
        self.emit_state_changed(app_handle, false);
    }

    #[cfg(target_os = "android")]
    fn try_android_auto_advance(&mut self, state: &Arc<Mutex<PlaybackState>>, app_handle: &AppHandle) -> bool {
        let domain = match app_handle.try_state::<PlaybackDomainState>() {
            Some(s) => s,
            None => {
                self.end_pending = false;
                self.is_playing = false;
                self.pause_target_secs = None;
                self.fade_state = FadeState::None;
                update_state(
                    state,
                    false,
                    self.duration_secs,
                    self.duration_secs,
                    self.volume,
                );
                return false;
            }
        };
        let engine = match app_handle.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return false,
        };

        if !playback_control::next(&domain, &engine) {
            return false;
        }

        let (index, track_id) = {
            let d = match domain.0.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    log::error!("Failed to lock domain: {}", e);
                    return false;
                }
            };
            if d.queue.is_empty() || d.index >= d.queue.len() {
                (0, String::new())
            } else {
                (d.index, d.queue[d.index].id.clone())
            }
        };
        if !track_id.is_empty() {
            let _ = app_handle.emit(
                "playback:domain_changed",
                serde_json::json!({ "index": index, "track_id": track_id }),
            );
        }
        true
    }

    /// Handle the Android auto-advance failure fallback. Returns `true` if `continue` needed.
    #[cfg(target_os = "android")]
    #[allow(dead_code)]
    fn android_auto_advance_fallback(
        end_pending: &mut bool,
        is_playing: &mut bool,
        pause_target_secs: &mut Option<f64>,
        fade_state: &mut FadeState,
        state: &Arc<Mutex<PlaybackState>>,
        duration_secs: f64,
        volume: f32,
        app_handle: &AppHandle,
    ) -> bool {
        *end_pending = false;
        *is_playing = false;
        *pause_target_secs = None;
        *fade_state = FadeState::None;
        update_state(state, false, duration_secs, duration_secs, volume);
        let _ = app_handle.emit("audio:ended", ());
        let _ = app_handle.emit(
            "audio:state_changed",
            StateChangedPayload { is_playing: false },
        );
        true
    }

    /// Emit time event ~4 Hz.
    fn emit_time_event(
        &mut self,
        state: &Arc<Mutex<PlaybackState>>,
        app_handle: &AppHandle,
    ) {
        if !self.is_playing || self.last_time_emit.elapsed() < Duration::from_millis(250) {
            return;
        }

        let playback_pos = self.playback_position();
        let playback_pos = if playback_pos + 0.03 < self.last_emitted_pos {
            self.last_emitted_pos
        } else {
            self.last_emitted_pos = playback_pos;
            playback_pos
        };

        update_state(
            state,
            self.is_playing,
            playback_pos,
            self.duration_secs,
            self.volume,
        );
        let _ = app_handle.emit(
            "audio:time",
            TimePayload {
                position: playback_pos,
                duration: self.duration_secs,
                session_id: self.session_id,
            },
        );
        self.last_time_emit = Instant::now();
    }

    /// Emit FFT event ~30 Hz.
    fn emit_fft_event(&mut self, app_handle: &AppHandle) {
        if !self.fft_proc.is_enabled() || self.last_fft_emit.elapsed() < Duration::from_millis(33) {
            return;
        }

        let (frequency, waveform) = self.fft_proc.compute();
        let _ = app_handle.emit("audio:fft", FftPayload { frequency, waveform });
        self.last_fft_emit = Instant::now();
    }

    fn sleep(&self) {
        if self.is_playing {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

// ── Main thread entry ───────────────────────────────────────────

fn audio_thread(
    cmd_rx: Receiver<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    app_handle: AppHandle,
) {
    let mut s = AudioThreadState::new();

    loop {
        s.handle_output_error(&state, &app_handle);
        s.handle_cmds(&cmd_rx, &state, &app_handle);

        let fade_completed = s.decode_and_feed(&app_handle);

        if fade_completed {
            s.handle_fade_completion(&state, &app_handle);
        }

        s.handle_natural_end(&state, &app_handle);

        s.emit_time_event(&state, &app_handle);
        s.emit_fft_event(&app_handle);
        s.sleep();
    }
}

// ── Free functions ──────────────────────────────────────────────

fn update_state(
    state: &Arc<Mutex<PlaybackState>>,
    is_playing: bool,
    position_secs: f64,
    duration_secs: f64,
    volume: f32,
) {
    if let Ok(mut s) = state.lock() {
        s.is_playing = is_playing;
        s.position_secs = position_secs;
        s.duration_secs = duration_secs;
        s.volume = volume;
    }
}

fn fade_step(duration_ms: f32, sample_rate: u32, channels: usize) -> f32 {
    1.0 / (duration_ms * 0.001 * sample_rate as f32 * channels as f32)
}

fn apply_volume_with_fade(samples: &mut [f32], volume: f32, fade: &mut FadeState) -> bool {
    match fade {
        FadeState::None => {
            for s in samples.iter_mut() {
                *s = (*s * volume).clamp(-1.0, 1.0);
            }
            false
        }
        FadeState::FadingIn { gain, step } => {
            for s in samples.iter_mut() {
                *s = (*s * volume * *gain).clamp(-1.0, 1.0);
                *gain = (*gain + *step).min(1.0);
            }
            if *gain >= 1.0 {
                *fade = FadeState::None;
            }
            false
        }
        FadeState::FadingOut { gain, step, .. } => {
            for s in samples.iter_mut() {
                *s = (*s * volume * *gain).clamp(-1.0, 1.0);
                *gain = (*gain - *step).max(0.0);
            }
            *gain <= 0.0
        }
    }
}

fn push_or_pend(producer: &mut HeapProd<f32>, samples: &[f32], pending: &mut Vec<f32>) {
    let written = producer.push_slice(samples);
    if written < samples.len() {
        if pending.len() < PENDING_SAMPLES_MAX {
            pending.extend_from_slice(&samples[written..]);
        } else {
            log::warn!(
                "Pending samples at capacity ({}), dropping {} samples",
                PENDING_SAMPLES_MAX,
                samples.len() - written
            );
        }
    }
}

fn convert_channels(samples: &[f32], from_ch: usize, to_ch: usize) -> Vec<f32> {
    if from_ch == to_ch {
        return samples.to_vec();
    }

    let frames = samples.len() / from_ch;
    let mut out = Vec::with_capacity(frames * to_ch);

    if from_ch == 1 && to_ch == 2 {
        for frame in 0..frames {
            let s = samples[frame];
            out.push(s);
            out.push(s);
        }
    } else if from_ch == 2 && to_ch == 1 {
        for frame in 0..frames {
            let l = samples[frame * 2];
            let r = samples[frame * 2 + 1];
            out.push((l + r) * 0.5);
        }
    } else if from_ch > 2 && to_ch == 2 {
        for frame in 0..frames {
            let base = frame * from_ch;
            let channels = &samples[base..base + from_ch];
            let (l, r) = downmix_to_stereo(channels);
            out.push(l);
            out.push(r);
        }
    } else if from_ch > to_ch {
        for frame in 0..frames {
            let base = frame * from_ch;
            for ch in 0..to_ch {
                let start = ch * from_ch / to_ch;
                let end = ((ch + 1) * from_ch / to_ch).max(start + 1);
                let mut sum = 0.0f32;
                let mut count = 0usize;
                for idx in start..end.min(from_ch) {
                    sum += samples[base + idx];
                    count += 1;
                }
                out.push(if count > 0 { sum / count as f32 } else { 0.0 });
            }
        }
    } else {
        for frame in 0..frames {
            for ch in 0..to_ch {
                let src_ch = ch.min(from_ch - 1);
                out.push(samples[frame * from_ch + src_ch]);
            }
        }
    }

    out
}

fn downmix_to_stereo(channels: &[f32]) -> (f32, f32) {
    const CENTER: f32 = 0.70710677;
    const SURROUND: f32 = 0.70710677;
    const BACK: f32 = 0.5;

    let get = |idx: usize| -> f32 { channels.get(idx).copied().unwrap_or(0.0) };

    match channels.len() {
        0 => (0.0, 0.0),
        1 => { let m = get(0); (m, m) }
        2 => (get(0), get(1)),
        3 => {
            let l = get(0) + get(2) * CENTER;
            let r = get(1) + get(2) * CENTER;
            (l, r)
        }
        4 => {
            let l = get(0) + get(2) * SURROUND;
            let r = get(1) + get(3) * SURROUND;
            (l, r)
        }
        5 => {
            let l = get(0) + get(2) * CENTER + get(3) * SURROUND;
            let r = get(1) + get(2) * CENTER + get(4) * SURROUND;
            (l, r)
        }
        6 => {
            let l = get(0) + get(2) * CENTER + get(4) * SURROUND;
            let r = get(1) + get(2) * CENTER + get(5) * SURROUND;
            (l, r)
        }
        7 => {
            let l = get(0) + get(2) * CENTER + get(4) * BACK + get(5) * SURROUND;
            let r = get(1) + get(2) * CENTER + get(4) * BACK + get(6) * SURROUND;
            (l, r)
        }
        _ => {
            let l = get(0) + get(2) * CENTER + get(4) * BACK + get(6) * SURROUND;
            let r = get(1) + get(2) * CENTER + get(5) * BACK + get(7) * SURROUND;
            (l, r)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{convert_channels, downmix_to_stereo};

    fn approx_eq(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "left={a}, right={b}");
    }

    #[test]
    fn downmixes_three_channel_center_into_both_sides() {
        let (l, r) = downmix_to_stereo(&[1.0, 2.0, 3.0]);
        approx_eq(l, 1.0 + 3.0 * 0.70710677);
        approx_eq(r, 2.0 + 3.0 * 0.70710677);
    }

    #[test]
    fn convert_channels_uses_multichannel_stereo_matrix_for_51() {
        let out = convert_channels(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 6, 2);
        assert_eq!(out.len(), 2);
        approx_eq(out[0], 1.0 + 3.0 * 0.70710677 + 5.0 * 0.70710677);
        approx_eq(out[1], 2.0 + 3.0 * 0.70710677 + 6.0 * 0.70710677);
    }
}
