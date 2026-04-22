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
const FADE_IN_MS: f32 = 200.0;

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

/// Commands sent from IPC to the audio thread.
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

/// Shared playback state readable from IPC.
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
}

// Event payloads
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
    /// Create a new engine + spawn the audio thread.
    pub fn new(app_handle: AppHandle) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
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
        let _ = self.cmd_tx.send(cmd);
    }
}

/// Open a new audio source, set up output/resampler/EQ, and optionally start with fade-in.
/// Returns true on success.
#[allow(clippy::too_many_arguments)]
fn execute_play(
    source: &str,
    with_fade_in: bool,
    decoder: &mut Option<AudioDecoder>,
    output: &mut Option<AudioOutput>,
    resampler: &mut Option<AudioResampler>,
    resample_buffer: &mut Vec<f32>,
    eq: &mut Equalizer,
    fade_state: &mut FadeState,
    source_sample_rate: &mut u32,
    source_channels: &mut usize,
    position_secs: &mut f64,
    duration_secs: &mut f64,
    is_playing: &mut bool,
    volume: f32,
    state: &Arc<Mutex<PlaybackState>>,
    app_handle: &AppHandle,
) -> bool {
    *decoder = None;
    *output = None;
    *resampler = None;
    resample_buffer.clear();
    *is_playing = false;
    *position_secs = 0.0;

    match AudioDecoder::open(source) {
        Ok(dec) => {
            *source_sample_rate = dec.info.sample_rate;
            *source_channels = dec.info.channels;
            *duration_secs = dec.info.duration_secs;

            let output_channels = (*source_channels).min(2) as u16;

            match AudioOutput::new(*source_sample_rate, output_channels) {
                Ok(out) => {
                    let out_rate = out.config.sample_rate.0;
                    let actual_channels = out.config.channels as usize;
                    if out_rate != *source_sample_rate {
                        match AudioResampler::new(
                            *source_sample_rate,
                            out_rate,
                            actual_channels,
                        ) {
                            Ok(rs) => *resampler = Some(rs),
                            Err(e) => {
                                eprintln!("Resampler init warning: {}", e);
                            }
                        }
                    }

                    let effective_rate = if resampler.is_some() { out_rate } else { *source_sample_rate };
                    {
                        let mut new_eq = Equalizer::new(effective_rate, actual_channels);
                        new_eq.set_enabled(eq.is_enabled());
                        std::mem::swap(eq, &mut new_eq);
                    }

                    let fade_rate = if resampler.is_some() { out_rate } else { *source_sample_rate };
                    let fade_ch = actual_channels;

                    *output = Some(out);
                    *decoder = Some(dec);
                    *is_playing = true;

                    if with_fade_in {
                        *fade_state = FadeState::FadingIn {
                            gain: 0.0,
                            step: fade_step(FADE_IN_MS, fade_rate, fade_ch),
                        };
                    } else {
                        *fade_state = FadeState::None;
                    }

                    update_state(state, *is_playing, *position_secs, *duration_secs, volume);
                    let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: true });
                    true
                }
                Err(e) => {
                    let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                    false
                }
            }
        }
        Err(e) => {
            let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reinitialize_output_for_route_change(
    dec: &mut AudioDecoder,
    output: &mut Option<AudioOutput>,
    resampler: &mut Option<AudioResampler>,
    resample_buffer: &mut Vec<f32>,
    pending_samples: &mut Vec<f32>,
    eq: &mut Equalizer,
    fade_state: &mut FadeState,
    source_sample_rate: u32,
    source_channels: usize,
    position_secs: &mut f64,
    last_emitted_pos: &mut f64,
    should_play: bool,
    duration_secs: f64,
    volume: f32,
    state: &Arc<Mutex<PlaybackState>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let target_position = current_playback_position(*position_secs, output);
    let output_channels = source_channels.min(2) as u16;

    let new_output = AudioOutput::new(source_sample_rate, output_channels)?;
    let out_rate = new_output.config.sample_rate.0;
    let actual_channels = new_output.config.channels as usize;

    dec.seek(target_position)
        .map_err(|e| format!("Failed to seek during output reinit: {e}"))?;

    *output = Some(new_output);
    if !should_play {
        if let Some(ref out) = output {
            out.pause();
        }
    }

    *resampler = None;
    resample_buffer.clear();
    pending_samples.clear();

    if out_rate != source_sample_rate {
        match AudioResampler::new(source_sample_rate, out_rate, actual_channels) {
            Ok(rs) => *resampler = Some(rs),
            Err(e) => {
                eprintln!("Resampler reinit warning after route change: {}", e);
            }
        }
    }

    let effective_rate = if resampler.is_some() {
        out_rate
    } else {
        source_sample_rate
    };
    let eq_enabled = eq.is_enabled();
    let eq_gains = eq.gains();
    let mut new_eq = Equalizer::new(effective_rate, actual_channels);
    new_eq.set_enabled(eq_enabled);
    new_eq.set_gains(&eq_gains);
    *eq = new_eq;

    *position_secs = target_position;
    *last_emitted_pos = target_position;
    *fade_state = if should_play {
        FadeState::FadingIn {
            gain: 0.0,
            step: fade_step(FADE_IN_MS, effective_rate, actual_channels),
        }
    } else {
        FadeState::None
    };

    update_state(state, should_play, *position_secs, duration_secs, volume);
    let _ = app_handle.emit(
        "audio:state_changed",
        StateChangedPayload {
            is_playing: should_play,
        },
    );
    Ok(())
}

fn audio_thread(
    cmd_rx: Receiver<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    app_handle: AppHandle,
) {
    let mut decoder: Option<AudioDecoder> = None;
    let mut output: Option<AudioOutput> = None;
    let mut eq = Equalizer::new(44100, 2);
    let mut fft_proc = FftProcessor::new();
    let mut resampler: Option<AudioResampler> = None;
    let mut resample_buffer: Vec<f32> = Vec::new();
    let mut pending_samples: Vec<f32> = Vec::new();

    let mut volume: f32 = 1.0;
    let mut position_secs: f64 = 0.0;
    let mut duration_secs: f64 = 0.0;
    let mut is_playing = false;
    let mut source_sample_rate: u32 = 44100;
    let mut source_channels: usize = 2;
    let mut fade_state = FadeState::None;
    // Decoder reached EOF; keep output running until its ring buffer drains,
    // then emit `audio:ended` so the UI doesn't advance early.
    let mut end_pending = false;
    let mut pause_target_secs: Option<f64> = None;

    let mut last_time_emit = Instant::now();
    let mut last_fft_emit = Instant::now();
    let mut session_id: u64 = 0;
    let mut last_emitted_pos: f64 = 0.0;

    loop {
        // 0. Auto-recover when the underlying output stream fails (e.g. route
        // switch / spatial effect toggle while playing).
        let output_failed = output
            .as_ref()
            .map(|out| out.has_stream_error())
            .unwrap_or(false);
        if output_failed {
            pause_target_secs = None;
            end_pending = false;

            if let Some(ref mut dec) = decoder {
                let should_play = is_playing;
                if let Err(e) = reinitialize_output_for_route_change(
                    dec,
                    &mut output,
                    &mut resampler,
                    &mut resample_buffer,
                    &mut pending_samples,
                    &mut eq,
                    &mut fade_state,
                    source_sample_rate,
                    source_channels,
                    &mut position_secs,
                    &mut last_emitted_pos,
                    should_play,
                    duration_secs,
                    volume,
                    &state,
                    &app_handle,
                ) {
                    is_playing = false;
                    output = None;
                    resampler = None;
                    resample_buffer.clear();
                    pending_samples.clear();
                    fade_state = FadeState::None;
                    update_state(&state, false, position_secs, duration_secs, volume);
                    let _ = app_handle.emit(
                        "audio:error",
                        ErrorPayload {
                            message: format!(
                                "Audio output stream failed and recovery failed: {}",
                                e
                            ),
                        },
                    );
                    let _ = app_handle.emit(
                        "audio:state_changed",
                        StateChangedPayload { is_playing: false },
                    );
                }
            } else {
                is_playing = false;
                output = None;
                resampler = None;
                resample_buffer.clear();
                pending_samples.clear();
                fade_state = FadeState::None;
                update_state(&state, false, position_secs, duration_secs, volume);
                let _ = app_handle.emit(
                    "audio:state_changed",
                    StateChangedPayload { is_playing: false },
                );
            }
        }

        // 1. Process all pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCommand::Play { source } => {
                    pause_target_secs = None;
                    let was_draining = end_pending;
                    // Any explicit play cancels a pending natural end/drain.
                    end_pending = false;
                    if is_playing {
                        if was_draining {
                            // We're only draining buffered samples (decoder EOF). There is no new decoded audio
                            // to apply a fade envelope to, so switch immediately.
                            if let Some(ref out) = output {
                                out.flush();
                            }
                            pending_samples.clear();
                            session_id = session_id.wrapping_add(1);
                            if session_id == 0 {
                                session_id = 1;
                            }
                            if execute_play(
                                &source,
                                true,
                                &mut decoder,
                                &mut output,
                                &mut resampler,
                                &mut resample_buffer,
                                &mut eq,
                                &mut fade_state,
                                &mut source_sample_rate,
                                &mut source_channels,
                                &mut position_secs,
                                &mut duration_secs,
                                &mut is_playing,
                                volume,
                                &state,
                                &app_handle,
                            ) {
                                last_emitted_pos = 0.0;
                            }
                            pending_samples.clear();
                            continue;
                        }
                        // Currently playing: fade out then switch
                        if let Some(ref out) = output {
                            out.flush();
                        }
                        let out_rate = output.as_ref().map(|o| o.config.sample_rate.0).unwrap_or(source_sample_rate);
                        let out_ch = output.as_ref().map(|o| o.config.channels as usize).unwrap_or(2);
                        let current_gain = match &fade_state {
                            FadeState::FadingIn { gain, .. } => *gain,
                            FadeState::FadingOut { gain, .. } => *gain,
                            FadeState::None => 1.0,
                        };
                        fade_state = FadeState::FadingOut {
                            gain: current_gain,
                            step: fade_step(FADE_OUT_MS, out_rate, out_ch),
                            action: FadeAction::PlayNext { source },
                        };
                    } else {
                        session_id = session_id.wrapping_add(1);
                        if session_id == 0 {
                            session_id = 1;
                        }
                        if execute_play(
                            &source, true,
                            &mut decoder, &mut output, &mut resampler, &mut resample_buffer,
                            &mut eq, &mut fade_state,
                            &mut source_sample_rate, &mut source_channels,
                            &mut position_secs, &mut duration_secs, &mut is_playing,
                            volume, &state, &app_handle,
                        ) {
                            last_emitted_pos = 0.0;
                        }
                        pending_samples.clear();
                    }
                }
                AudioCommand::Pause => {
                    if is_playing {
                        pause_target_secs = Some(current_playback_position(position_secs, &output));
                        if end_pending {
                            // EOF reached: we're only draining buffered samples, so pause immediately.
                            is_playing = false;
                            if let Some(ref out) = output {
                                out.pause();
                            }
                            if let Some(target) = pause_target_secs.take() {
                                position_secs = target;
                                last_emitted_pos = target;
                            }
                            update_state(&state, false, position_secs, duration_secs, volume);
                            let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                            continue;
                        }
                        let out_rate = output.as_ref().map(|o| o.config.sample_rate.0).unwrap_or(source_sample_rate);
                        let out_ch = output.as_ref().map(|o| o.config.channels as usize).unwrap_or(2);
                        let current_gain = match &fade_state {
                            FadeState::FadingIn { gain, .. } => *gain,
                            FadeState::FadingOut { gain, .. } => *gain,
                            FadeState::None => 1.0,
                        };
                        fade_state = FadeState::FadingOut {
                            gain: current_gain,
                            step: fade_step(FADE_OUT_MS, out_rate, out_ch),
                            action: FadeAction::Pause,
                        };
                    }
                }
                AudioCommand::Resume => {
                    if !is_playing && decoder.is_some() {
                        pause_target_secs = None;
                        is_playing = true;
                        if let Some(ref out) = output {
                            out.resume();
                        }
                        let out_rate = output.as_ref().map(|o| o.config.sample_rate.0).unwrap_or(source_sample_rate);
                        let out_ch = output.as_ref().map(|o| o.config.channels as usize).unwrap_or(2);
                        fade_state = FadeState::FadingIn {
                            gain: 0.0,
                            step: fade_step(FADE_IN_MS, out_rate, out_ch),
                        };
                        update_state(&state, true, position_secs, duration_secs, volume);
                        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: true });
                    } else if is_playing {
                        // Currently fading out for a pause — reverse into fade-in
                        if let FadeState::FadingOut { gain, action: FadeAction::Pause, .. } = &fade_state {
                            pause_target_secs = None;
                            let current_gain = *gain;
                            let out_rate = output.as_ref().map(|o| o.config.sample_rate.0).unwrap_or(source_sample_rate);
                            let out_ch = output.as_ref().map(|o| o.config.channels as usize).unwrap_or(2);
                            fade_state = FadeState::FadingIn {
                                gain: current_gain,
                                step: fade_step(FADE_IN_MS, out_rate, out_ch),
                            };
                        }
                    }
                }
                AudioCommand::Stop => {
                    pause_target_secs = None;
                    let was_draining = end_pending;
                    // Any explicit stop cancels a pending natural end/drain.
                    end_pending = false;
                    if is_playing {
                        if was_draining {
                            // Only draining buffered samples (decoder EOF): stop immediately (no fade possible).
                            decoder = None;
                            output = None;
                            resampler = None;
                            resample_buffer.clear();
                            pending_samples.clear();
                            is_playing = false;
                            position_secs = 0.0;
                            duration_secs = 0.0;
                            fade_state = FadeState::None;
                            fft_proc.set_enabled(false);
                            update_state(&state, false, 0.0, 0.0, volume);
                            let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                            continue;
                        }
                        if let Some(ref out) = output {
                            out.flush();
                        }
                        let out_rate = output.as_ref().map(|o| o.config.sample_rate.0).unwrap_or(source_sample_rate);
                        let out_ch = output.as_ref().map(|o| o.config.channels as usize).unwrap_or(2);
                        let current_gain = match &fade_state {
                            FadeState::FadingIn { gain, .. } => *gain,
                            FadeState::FadingOut { gain, .. } => *gain,
                            FadeState::None => 1.0,
                        };
                        fade_state = FadeState::FadingOut {
                            gain: current_gain,
                            step: fade_step(FADE_OUT_MS, out_rate, out_ch),
                            action: FadeAction::Stop,
                        };
                    } else {
                        decoder = None;
                        output = None;
                        resampler = None;
                        resample_buffer.clear();
                        pending_samples.clear();
                        position_secs = 0.0;
                        duration_secs = 0.0;
                        fade_state = FadeState::None;
                        fft_proc.set_enabled(false);
                        update_state(&state, false, 0.0, 0.0, volume);
                        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                    }
                }
                AudioCommand::Seek { position_secs: pos } => {
                    pause_target_secs = None;
                    // Seeking during drain should resume decoding from the new position.
                    end_pending = false;
                    if let Some(ref mut dec) = decoder {
                        let clamped = if duration_secs > 0.0 {
                            pos.clamp(0.0, duration_secs)
                        } else {
                            pos.max(0.0)
                        };
                        if let Err(e) = dec.seek(clamped) {
                            eprintln!("Seek error: {}", e);
                        } else {
                            position_secs = clamped;
                            last_emitted_pos = clamped;
                            if let Some(ref out) = output {
                                out.flush();
                            }
                            eq.reset();
                            pending_samples.clear();
                            update_state(&state, is_playing, position_secs, duration_secs, volume);
                        }
                    }
                }
                #[cfg(target_os = "android")]
                AudioCommand::RefreshOutput => {
                    pause_target_secs = None;
                    end_pending = false;

                    let Some(ref mut dec) = decoder else {
                        continue;
                    };

                    let should_play = is_playing;
                    if let Err(e) = reinitialize_output_for_route_change(
                        dec,
                        &mut output,
                        &mut resampler,
                        &mut resample_buffer,
                        &mut pending_samples,
                        &mut eq,
                        &mut fade_state,
                        source_sample_rate,
                        source_channels,
                        &mut position_secs,
                        &mut last_emitted_pos,
                        should_play,
                        duration_secs,
                        volume,
                        &state,
                        &app_handle,
                    ) {
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
                AudioCommand::SetVolume { volume: vol } => {
                    volume = vol.clamp(0.0, 1.0);
                    update_state(&state, is_playing, position_secs, duration_secs, volume);
                }
                AudioCommand::SetEqBands { gains } => {
                    eq.set_gains(&gains);
                }
                AudioCommand::SetEqEnabled { enabled } => {
                    eq.set_enabled(enabled);
                }
                AudioCommand::EnableVisualization { enabled } => {
                    fft_proc.set_enabled(enabled);
                }
            }
        }

        // 2. If playing, decode and feed output
        let mut fade_completed = false;
        if is_playing {
            if let (Some(ref mut dec), Some(ref mut out)) = (&mut decoder, &mut output) {
                let out_channels = out.config.channels as usize;

                // Flush any pending samples from previous iteration
                if !pending_samples.is_empty() {
                    let written = out.producer.push_slice(&pending_samples);
                    if written > 0 {
                        pending_samples.drain(..written);
                    }
                }

                for _ in 0..32 {
                    if end_pending {
                        break;
                    }
                    // Don't decode more if we still have unsent samples
                    if !pending_samples.is_empty() {
                        break;
                    }
                    let available = out.producer.vacant_len();
                    if available < 8192 {
                        break;
                    }

                    match dec.decode_next() {
                        Ok(Some(mut samples)) => {
                            let decoded_channels = source_channels;
                            let decoded_frames = samples.len() / decoded_channels;

                            if decoded_channels != out_channels {
                                samples = convert_channels(&samples, decoded_channels, out_channels);
                            }

                            if let Some(ref mut rs) = resampler {
                                resample_buffer.extend_from_slice(&samples);
                                let needed = rs.input_frames_needed() * out_channels;
                                while resample_buffer.len() >= needed && pending_samples.is_empty() {
                                    let chunk: Vec<f32> = resample_buffer.drain(..needed).collect();
                                    match rs.process(&chunk) {
                                        Ok(resampled) => {
                                            let mut resampled = resampled;
                                            eq.process(&mut resampled);
                                            fft_proc.push_samples(&resampled, out_channels);
                                            if apply_volume_with_fade(&mut resampled, volume, &mut fade_state) {
                                                push_or_pend(&mut out.producer, &resampled, &mut pending_samples);
                                                fade_completed = true;
                                                break;
                                            }
                                            push_or_pend(&mut out.producer, &resampled, &mut pending_samples);
                                        }
                                        Err(e) => {
                                            eprintln!("Resample error: {}", e);
                                        }
                                    }
                                    let next_needed = rs.input_frames_needed() * out_channels;
                                    if resample_buffer.len() < next_needed {
                                        break;
                                    }
                                }
                            } else {
                                eq.process(&mut samples);
                                fft_proc.push_samples(&samples, out_channels);
                                if apply_volume_with_fade(&mut samples, volume, &mut fade_state) {
                                    push_or_pend(&mut out.producer, &samples, &mut pending_samples);
                                    fade_completed = true;
                                }
                                if !fade_completed {
                                    push_or_pend(&mut out.producer, &samples, &mut pending_samples);
                                }
                            }

                            if fade_completed {
                                break;
                            }

                            position_secs += decoded_frames as f64 / source_sample_rate as f64;
                            if position_secs > duration_secs && duration_secs > 0.0 {
                                position_secs = duration_secs;
                            }
                        }
                        Ok(None) => {
                            // End of stream — use accumulated position as true duration
                            // if the initial duration was unknown or suspiciously off
                            if duration_secs <= 0.0 || (position_secs - duration_secs).abs() > 1.0 {
                                duration_secs = position_secs;
                            }
                            // Mark end-of-stream, but wait to notify the UI until the output buffer drains.
                            end_pending = true;
                            break;
                        }
                        Err(e) => {
                            is_playing = false;
                            fade_state = FadeState::None;
                            let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                            break;
                        }
                    }
                }
            }
        }

        // 3. Handle fade-out completion
        if fade_completed {
            // Take ownership of the action from fade_state
            let action = std::mem::replace(&mut fade_state, FadeState::None);
            match action {
                FadeState::FadingOut { action, .. } => match action {
                    FadeAction::Pause => {
                        is_playing = false;
                        if let Some(ref out) = output {
                            out.pause();
                        }
                        if let Some(target) = pause_target_secs.take() {
                            if let Some(ref mut dec) = decoder {
                                if let Err(e) = dec.seek(target) {
                                    eprintln!("Pause seek-back error: {}", e);
                                } else {
                                    position_secs = target;
                                    last_emitted_pos = target;
                                    if let Some(ref out) = output {
                                        out.flush();
                                    }
                                    eq.reset();
                                    pending_samples.clear();
                                }
                            } else {
                                position_secs = target;
                                last_emitted_pos = target;
                            }
                        }
                        update_state(&state, false, position_secs, duration_secs, volume);
                        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                    }
                    FadeAction::Stop => {
                        pause_target_secs = None;
                        decoder = None;
                        output = None;
                        resampler = None;
                        resample_buffer.clear();
                        pending_samples.clear();
                        is_playing = false;
                        position_secs = 0.0;
                        duration_secs = 0.0;
                        last_emitted_pos = 0.0;
                        fade_state = FadeState::None;
                        fft_proc.set_enabled(false);
                        update_state(&state, false, 0.0, 0.0, volume);
                        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                    }
                    FadeAction::PlayNext { source } => {
                        pause_target_secs = None;
                        session_id = session_id.wrapping_add(1);
                        if session_id == 0 {
                            session_id = 1;
                        }
                        if execute_play(
                            &source, true,
                            &mut decoder, &mut output, &mut resampler, &mut resample_buffer,
                            &mut eq, &mut fade_state,
                            &mut source_sample_rate, &mut source_channels,
                            &mut position_secs, &mut duration_secs, &mut is_playing,
                            volume, &state, &app_handle,
                        ) {
                            last_emitted_pos = 0.0;
                        }
                        pending_samples.clear();
                    }
                },
                _ => {}
            }
        }

        // 3b. Natural end: only emit `audio:ended` once the output ring buffer drains.
        if end_pending {
            let buffered = output
                .as_ref()
                .map(|o| o.producer.occupied_len())
                .unwrap_or(0);
            if pending_samples.is_empty() && buffered == 0 {
                // On Android we handle "ended -> next track" in Rust so playback can continue
                // when the WebView is backgrounded or suspended.
                #[cfg(target_os = "android")]
                {
                    let domain = match app_handle.try_state::<PlaybackDomainState>() {
                        Some(s) => s,
                        None => {
                            // App state not ready; fall back to ended.
                            end_pending = false;
                            is_playing = false;
                            pause_target_secs = None;
                            fade_state = FadeState::None;
                            update_state(&state, false, duration_secs, duration_secs, volume);
                            let _ = app_handle.emit("audio:ended", ());
                            let _ = app_handle.emit(
                                "audio:state_changed",
                                StateChangedPayload { is_playing: false },
                            );
                            continue;
                        }
                    };
                    let engine = match app_handle.try_state::<AudioEngineState>() {
                        Some(s) => s,
                        None => {
                            end_pending = false;
                            is_playing = false;
                            pause_target_secs = None;
                            fade_state = FadeState::None;
                            update_state(&state, false, duration_secs, duration_secs, volume);
                            let _ = app_handle.emit("audio:ended", ());
                            let _ = app_handle.emit(
                                "audio:state_changed",
                                StateChangedPayload { is_playing: false },
                            );
                            continue;
                        }
                    };

                    if playback_control::next(&domain, &engine) {
                        // Notify UI (when alive) that the playback domain advanced.
                        let (index, track_id) = {
                            let d = domain.0.lock().unwrap();
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

                        // Do not emit `audio:ended` when we auto-advance.
                        pending_samples.clear();
                        continue;
                    }
                }

                // No auto-advance (or not on Android): end playback and notify UI.
                end_pending = false;
                is_playing = false;
                pause_target_secs = None;
                fade_state = FadeState::None;
                update_state(&state, false, duration_secs, duration_secs, volume);
                let _ = app_handle.emit("audio:ended", ());
                let _ = app_handle.emit(
                    "audio:state_changed",
                    StateChangedPayload { is_playing: false },
                );
            }
        }

        // 4. Emit time event ~4Hz
        if is_playing && last_time_emit.elapsed() >= Duration::from_millis(250) {
            let playback_pos = if let Some(ref out) = output {
                let buffered_samples = out.producer.occupied_len();
                let out_rate = out.config.sample_rate.0 as f64;
                let out_ch = out.config.channels as f64;
                let buffered_secs = buffered_samples as f64 / (out_rate * out_ch);
                (position_secs - buffered_secs).max(0.0)
            } else {
                position_secs
            };
            let playback_pos = if playback_pos + 0.03 < last_emitted_pos {
                last_emitted_pos
            } else {
                last_emitted_pos = playback_pos;
                playback_pos
            };

            update_state(&state, is_playing, playback_pos, duration_secs, volume);
            let _ = app_handle.emit(
                "audio:time",
                TimePayload {
                    position: playback_pos,
                    duration: duration_secs,
                    session_id,
                },
            );
            last_time_emit = Instant::now();
        }

        // 5. Emit FFT event ~30Hz
        if fft_proc.is_enabled() && last_fft_emit.elapsed() >= Duration::from_millis(33) {
            let (frequency, waveform) = fft_proc.compute();
            let _ = app_handle.emit(
                "audio:fft",
                FftPayload {
                    frequency,
                    waveform,
                },
            );
            last_fft_emit = Instant::now();
        }

        // 6. Sleep to avoid busy-waiting
        if is_playing {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

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

fn current_playback_position(position_secs: f64, output: &Option<AudioOutput>) -> f64 {
    if let Some(out) = output.as_ref() {
        let buffered_samples = out.producer.occupied_len();
        let out_rate = out.config.sample_rate.0 as f64;
        let out_ch = out.config.channels as f64;
        if out_rate > 0.0 && out_ch > 0.0 {
            let buffered_secs = buffered_samples as f64 / (out_rate * out_ch);
            return (position_secs - buffered_secs).max(0.0);
        }
    }
    position_secs.max(0.0)
}

fn fade_step(duration_ms: f32, sample_rate: u32, channels: usize) -> f32 {
    1.0 / (duration_ms * 0.001 * sample_rate as f32 * channels as f32)
}

/// Apply volume and fade envelope per-sample. Returns `true` when a fade-out reaches 0.0.
/// Always clamps output to [-1.0, 1.0] to prevent DAC clipping.
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

/// Push samples into the ring buffer. Any overflow is saved to `pending` for the next iteration.
fn push_or_pend(producer: &mut HeapProd<f32>, samples: &[f32], pending: &mut Vec<f32>) {
    let written = producer.push_slice(samples);
    if written < samples.len() {
        pending.extend_from_slice(&samples[written..]);
    }
}

/// Convert between channel counts.
///
/// For multichannel -> stereo we use a conservative Lo/Ro downmix matrix
/// instead of just truncating to the first two channels. This preserves center
/// dialogue / vocals and surround ambience much better for desktop stereo
/// playback while keeping the implementation deterministic.
fn convert_channels(samples: &[f32], from_ch: usize, to_ch: usize) -> Vec<f32> {
    if from_ch == to_ch {
        return samples.to_vec();
    }

    let frames = samples.len() / from_ch;
    let mut out = Vec::with_capacity(frames * to_ch);

    if from_ch == 1 && to_ch == 2 {
        // Mono to stereo
        for frame in 0..frames {
            let s = samples[frame];
            out.push(s);
            out.push(s);
        }
    } else if from_ch == 2 && to_ch == 1 {
        // Stereo to mono
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
        // Generic downmix fallback: average grouped source channels.
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
        // Upmix: duplicate first channel into extra channels
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
    const CENTER: f32 = 0.70710677; // -3 dB
    const SURROUND: f32 = 0.70710677; // -3 dB
    const BACK: f32 = 0.5; // slightly lower than side surrounds

    let get = |idx: usize| -> f32 { channels.get(idx).copied().unwrap_or(0.0) };

    match channels.len() {
        0 => (0.0, 0.0),
        1 => {
            let m = get(0);
            (m, m)
        }
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
            // Assumed order: FL, FR, FC, LFE, SL, SR.
            // LFE is intentionally omitted here to avoid muddy stereo fold-down.
            let l = get(0) + get(2) * CENTER + get(4) * SURROUND;
            let r = get(1) + get(2) * CENTER + get(5) * SURROUND;
            (l, r)
        }
        7 => {
            // Assumed order: FL, FR, FC, LFE, BC, SL, SR.
            let l = get(0) + get(2) * CENTER + get(4) * BACK + get(5) * SURROUND;
            let r = get(1) + get(2) * CENTER + get(4) * BACK + get(6) * SURROUND;
            (l, r)
        }
        _ => {
            // Assumed order: FL, FR, FC, LFE, BL, BR, SL, SR, ...
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
