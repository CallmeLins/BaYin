use crossbeam_channel::{Receiver, Sender};
use ringbuf::traits::{Observer, Producer};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::decoder::AudioDecoder;
use super::dsp::Equalizer;
use super::fft::FftProcessor;
use super::output::AudioOutput;
use super::resampler::AudioResampler;

/// Commands sent from IPC to the audio thread.
pub enum AudioCommand {
    Play { source: String },
    Pause,
    Resume,
    Stop,
    Seek { position_secs: f64 },
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

    let mut volume: f32 = 1.0;
    let mut position_secs: f64 = 0.0;
    let mut duration_secs: f64 = 0.0;
    let mut is_playing = false;
    let mut source_sample_rate: u32 = 44100;
    let mut source_channels: usize = 2;

    let mut last_time_emit = Instant::now();
    let mut last_fft_emit = Instant::now();

    loop {
        // 1. Process all pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCommand::Play { source } => {
                    // Stop current playback
                    decoder = None;
                    output = None;
                    resampler = None;
                    resample_buffer.clear();
                    is_playing = false;
                    position_secs = 0.0;

                    match AudioDecoder::open(&source) {
                        Ok(dec) => {
                            source_sample_rate = dec.info.sample_rate;
                            source_channels = dec.info.channels;
                            duration_secs = dec.info.duration_secs;

                            let output_channels = source_channels.min(2) as u16;

                            // Try to open output at source rate
                            match AudioOutput::new(source_sample_rate, output_channels) {
                                Ok(out) => {
                                    let out_rate = out.config.sample_rate.0;
                                    // Set up resampler if needed
                                    if out_rate != source_sample_rate {
                                        match AudioResampler::new(
                                            source_sample_rate,
                                            out_rate,
                                            output_channels as usize,
                                        ) {
                                            Ok(rs) => resampler = Some(rs),
                                            Err(e) => {
                                                eprintln!("Resampler init warning: {}", e);
                                            }
                                        }
                                    }

                                    // Re-init EQ with correct rate
                                    let effective_rate = if resampler.is_some() { out_rate } else { source_sample_rate };
                                    {
                                        let mut new_eq = Equalizer::new(effective_rate, output_channels as usize);
                                        new_eq.set_enabled(eq.is_enabled());
                                        std::mem::swap(&mut eq, &mut new_eq);
                                    }

                                    output = Some(out);
                                    decoder = Some(dec);
                                    is_playing = true;

                                    update_state(&state, is_playing, position_secs, duration_secs, volume);
                                    let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: true });
                                }
                                Err(e) => {
                                    let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                                }
                            }
                        }
                        Err(e) => {
                            let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                        }
                    }
                }
                AudioCommand::Pause => {
                    if is_playing {
                        is_playing = false;
                        if let Some(ref out) = output {
                            out.pause();
                        }
                        update_state(&state, false, position_secs, duration_secs, volume);
                        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                    }
                }
                AudioCommand::Resume => {
                    if !is_playing && decoder.is_some() {
                        is_playing = true;
                        if let Some(ref out) = output {
                            out.resume();
                        }
                        update_state(&state, true, position_secs, duration_secs, volume);
                        let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: true });
                    }
                }
                AudioCommand::Stop => {
                    decoder = None;
                    output = None;
                    resampler = None;
                    resample_buffer.clear();
                    is_playing = false;
                    position_secs = 0.0;
                    duration_secs = 0.0;
                    fft_proc.set_enabled(false);
                    update_state(&state, false, 0.0, 0.0, volume);
                    let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                }
                AudioCommand::Seek { position_secs: pos } => {
                    if let Some(ref mut dec) = decoder {
                        if let Err(e) = dec.seek(pos) {
                            eprintln!("Seek error: {}", e);
                        } else {
                            position_secs = pos;
                            // Clear ring buffer by dropping and recreating output
                            // Actually just update position; ring buffer will flush naturally
                            eq.reset();
                            update_state(&state, is_playing, position_secs, duration_secs, volume);
                        }
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
        // Decode multiple packets per iteration to keep the ring buffer well-fed
        // and avoid underruns that cause crackling.
        if is_playing {
            if let (Some(ref mut dec), Some(ref mut out)) = (&mut decoder, &mut output) {
                let out_channels = out.config.channels as usize;

                // Decode up to 32 packets per tick to fill the buffer aggressively
                for _ in 0..32 {
                    let available = out.producer.vacant_len();
                    // Stop when less than ~8192 samples of space remain,
                    // ensuring any single decoded packet can fit without dropping data
                    if available < 8192 {
                        break;
                    }

                    match dec.decode_next() {
                        Ok(Some(mut samples)) => {
                            let decoded_channels = source_channels;

                            // Track decoded frames for position (always at source rate)
                            let decoded_frames = samples.len() / decoded_channels;

                            // Channel conversion if needed
                            if decoded_channels != out_channels {
                                samples = convert_channels(&samples, decoded_channels, out_channels);
                            }

                            // Resample if needed
                            if let Some(ref mut rs) = resampler {
                                resample_buffer.extend_from_slice(&samples);
                                let needed = rs.input_frames_needed() * out_channels;
                                while resample_buffer.len() >= needed {
                                    let chunk: Vec<f32> = resample_buffer.drain(..needed).collect();
                                    match rs.process(&chunk) {
                                        Ok(resampled) => {
                                            let mut resampled = resampled;
                                            eq.process(&mut resampled);
                                            fft_proc.push_samples(&resampled, out_channels);
                                            apply_volume(&mut resampled, volume);
                                            out.producer.push_slice(&resampled);
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
                                // No resampling needed
                                eq.process(&mut samples);
                                fft_proc.push_samples(&samples, out_channels);
                                apply_volume(&mut samples, volume);
                                out.producer.push_slice(&samples);
                            }

                            // Update position based on decoded frames at source rate
                            position_secs += decoded_frames as f64 / source_sample_rate as f64;
                            if position_secs > duration_secs && duration_secs > 0.0 {
                                position_secs = duration_secs;
                            }
                        }
                        Ok(None) => {
                            // End of stream
                            is_playing = false;
                            update_state(&state, false, duration_secs, duration_secs, volume);
                            let _ = app_handle.emit("audio:ended", ());
                            let _ = app_handle.emit("audio:state_changed", StateChangedPayload { is_playing: false });
                            break;
                        }
                        Err(e) => {
                            is_playing = false;
                            let _ = app_handle.emit("audio:error", ErrorPayload { message: e });
                            break;
                        }
                    }
                }
            }
        }

        // 3. Emit time event ~4Hz
        if is_playing && last_time_emit.elapsed() >= Duration::from_millis(250) {
            // Compensate for audio buffered in the ring buffer but not yet played
            let playback_pos = if let Some(ref out) = output {
                let buffered_samples = out.producer.occupied_len();
                let out_rate = out.config.sample_rate.0 as f64;
                let out_ch = out.config.channels as f64;
                let buffered_secs = buffered_samples as f64 / (out_rate * out_ch);
                (position_secs - buffered_secs).max(0.0)
            } else {
                position_secs
            };

            update_state(&state, is_playing, playback_pos, duration_secs, volume);
            let _ = app_handle.emit(
                "audio:time",
                TimePayload {
                    position: playback_pos,
                    duration: duration_secs,
                },
            );
            last_time_emit = Instant::now();
        }

        // 4. Emit FFT event ~30Hz
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

        // 5. Sleep to avoid busy-waiting
        if is_playing {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            // When not playing, sleep longer
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

fn apply_volume(samples: &mut [f32], volume: f32) {
    if (volume - 1.0).abs() > f32::EPSILON {
        for s in samples.iter_mut() {
            *s *= volume;
        }
    }
}

/// Convert between channel counts (mono<->stereo).
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
    } else if from_ch > to_ch {
        // Downmix: average first to_ch channels
        for frame in 0..frames {
            for ch in 0..to_ch {
                out.push(samples[frame * from_ch + ch]);
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
