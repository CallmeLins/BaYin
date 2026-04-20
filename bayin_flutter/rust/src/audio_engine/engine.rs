use crossbeam_channel::{bounded, Receiver, Sender};
use ringbuf::traits::{Observer, Producer};
use ringbuf::HeapProd;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::decoder::AudioDecoder;
use super::dsp::Equalizer;
use super::fft::FftProcessor;
use super::output::AudioOutput;
use super::resampler::AudioResampler;

type CommandReply = Sender<Result<(), String>>;

pub enum AudioCommand {
    Play {
        source: String,
        reply: CommandReply,
    },
    Pause {
        reply: CommandReply,
    },
    Resume {
        reply: CommandReply,
    },
    Stop {
        reply: CommandReply,
    },
    Seek {
        position_secs: f64,
        reply: CommandReply,
    },
    SetVolume {
        volume: f32,
        reply: CommandReply,
    },
    SetEqEnabled {
        enabled: bool,
        reply: CommandReply,
    },
    SetEqGains {
        gains: [f32; 10],
        reply: CommandReply,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub current_source: Option<String>,
    pub has_ended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FftSnapshot {
    pub frequency: Vec<u8>,
    pub waveform: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EqState {
    pub enabled: bool,
    pub gains: [f32; 10],
}

pub struct AudioEngine {
    cmd_tx: Sender<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    fft: Arc<Mutex<FftSnapshot>>,
    eq: Arc<Mutex<EqState>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let state = Arc::new(Mutex::new(PlaybackState {
            is_playing: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 1.0,
            current_source: None,
            has_ended: false,
        }));
        let state_clone = Arc::clone(&state);
        let fft = Arc::new(Mutex::new(FftSnapshot {
            frequency: vec![0; 64],
            waveform: vec![128; 128],
        }));
        let fft_clone = Arc::clone(&fft);
        let eq = Arc::new(Mutex::new(EqState {
            enabled: false,
            gains: [0.0; 10],
        }));
        let eq_clone = Arc::clone(&eq);

        std::thread::Builder::new()
            .name("audio-engine".to_string())
            .spawn(move || {
                audio_thread(cmd_rx, state_clone, fft_clone, eq_clone);
            })
            .map_err(|err| format!("Failed to spawn audio engine thread: {err}"))?;

        Ok(Self {
            cmd_tx,
            state,
            fft,
            eq,
        })
    }

    pub fn play(&self, source: String) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::Play { source, reply })
    }

    pub fn pause(&self) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::Pause { reply })
    }

    pub fn resume(&self) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::Resume { reply })
    }

    pub fn stop(&self) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::Stop { reply })
    }

    pub fn seek(&self, position_secs: f64) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::Seek {
            position_secs,
            reply,
        })
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::SetVolume { volume, reply })
    }

    pub fn set_eq_enabled(&self, enabled: bool) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::SetEqEnabled { enabled, reply })
    }

    pub fn set_eq_gains(&self, gains: [f32; 10]) -> Result<(), String> {
        self.send_wait(|reply| AudioCommand::SetEqGains { gains, reply })
    }

    pub fn playback_state(&self) -> Result<PlaybackState, String> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|err| err.to_string())
    }

    pub fn fft_snapshot(&self) -> Result<FftSnapshot, String> {
        self.fft
            .lock()
            .map(|fft| fft.clone())
            .map_err(|err| err.to_string())
    }

    pub fn eq_state(&self) -> Result<EqState, String> {
        self.eq
            .lock()
            .map(|eq| eq.clone())
            .map_err(|err| err.to_string())
    }

    fn send_wait(&self, build: impl FnOnce(CommandReply) -> AudioCommand) -> Result<(), String> {
        let (reply_tx, reply_rx) = bounded(1);
        let command = build(reply_tx);

        self.cmd_tx
            .send(command)
            .map_err(|err| format!("Failed to send audio command: {err}"))?;

        reply_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|err| format!("Audio command timed out: {err}"))?
    }
}

struct PreparedPlayback {
    decoder: AudioDecoder,
    output: AudioOutput,
    resampler: Option<AudioResampler>,
    equalizer: Equalizer,
    source_sample_rate: u32,
    source_channels: usize,
    duration_secs: f64,
}

fn audio_thread(
    cmd_rx: Receiver<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    fft_state: Arc<Mutex<FftSnapshot>>,
    eq_state: Arc<Mutex<EqState>>,
) {
    let mut decoder: Option<AudioDecoder> = None;
    let mut output: Option<AudioOutput> = None;
    let mut resampler: Option<AudioResampler> = None;
    let mut resample_buffer: Vec<f32> = Vec::new();
    let mut pending_samples: Vec<f32> = Vec::new();
    let mut source_sample_rate = 44_100u32;
    let mut source_channels = 2usize;
    let mut equalizer: Option<Equalizer> = None;
    let mut eq_enabled = false;
    let mut eq_gains = [0.0f32; 10];
    let mut decoded_position_secs = 0.0f64;
    let mut duration_secs = 0.0f64;
    let mut is_playing = false;
    let mut volume = 1.0f32;
    let mut current_source: Option<String> = None;
    let mut end_pending = false;
    let mut fft_processor = FftProcessor::new();
    fft_processor.set_enabled(true);
    let mut last_fft_emit = Instant::now();

    loop {
        while let Ok(command) = cmd_rx.try_recv() {
            match command {
                AudioCommand::Play { source, reply } => {
                    let result = prepare_playback(&source).map(|prepared| {
                        decoder = Some(prepared.decoder);
                        output = Some(prepared.output);
                        resampler = prepared.resampler;
                        equalizer = Some(prepared.equalizer);
                        if let Some(eq) = equalizer.as_mut() {
                            eq.set_enabled(eq_enabled);
                            eq.set_gains(&eq_gains);
                            eq.reset();
                        }
                        resample_buffer.clear();
                        pending_samples.clear();
                        source_sample_rate = prepared.source_sample_rate;
                        source_channels = prepared.source_channels;
                        decoded_position_secs = 0.0;
                        duration_secs = prepared.duration_secs;
                        is_playing = true;
                        end_pending = false;
                        current_source = Some(source);
                        update_state(
                            &state,
                            is_playing,
                            0.0,
                            duration_secs,
                            volume,
                            current_source.clone(),
                            false,
                        );
                    });
                    let _ = reply.send(result);
                }
                AudioCommand::Pause { reply } => {
                    let result = pause_playback(
                        &mut decoder,
                        output.as_ref(),
                        &mut resampler,
                        &mut equalizer,
                        &mut resample_buffer,
                        &mut pending_samples,
                        &mut decoded_position_secs,
                        duration_secs,
                        &mut is_playing,
                    )
                    .map(|position_secs| {
                        end_pending = false;
                        update_state(
                            &state,
                            false,
                            position_secs,
                            duration_secs,
                            volume,
                            current_source.clone(),
                            false,
                        );
                    });
                    let _ = reply.send(result);
                }
                AudioCommand::Resume { reply } => {
                    let result = if current_source.is_none() || output.is_none() {
                        Err("Nothing to resume".to_string())
                    } else {
                        if let Some(out) = output.as_ref() {
                            out.resume();
                        }
                        is_playing = true;
                        end_pending = false;
                        update_state(
                            &state,
                            true,
                            decoded_position_secs.max(0.0),
                            duration_secs,
                            volume,
                            current_source.clone(),
                            false,
                        );
                        Ok(())
                    };
                    let _ = reply.send(result);
                }
                AudioCommand::Stop { reply } => {
                    stop_playback(
                        &mut decoder,
                        &mut output,
                        &mut resampler,
                        &mut equalizer,
                        &mut resample_buffer,
                        &mut pending_samples,
                        &mut decoded_position_secs,
                        &mut duration_secs,
                        &mut is_playing,
                        &mut current_source,
                        &mut end_pending,
                    );
                    update_state(&state, false, 0.0, 0.0, volume, None, false);
                    let _ = reply.send(Ok(()));
                }
                AudioCommand::Seek {
                    position_secs,
                    reply,
                } => {
                    let result = seek_playback(
                        decoder.as_mut(),
                        output.as_ref(),
                        &mut resampler,
                        &mut equalizer,
                        &mut resample_buffer,
                        &mut pending_samples,
                        &mut decoded_position_secs,
                        duration_secs,
                        position_secs,
                    )
                    .map(|position_secs| {
                        end_pending = false;
                        update_state(
                            &state,
                            is_playing,
                            position_secs,
                            duration_secs,
                            volume,
                            current_source.clone(),
                            false,
                        );
                    });
                    let _ = reply.send(result);
                }
                AudioCommand::SetVolume {
                    volume: new_volume,
                    reply,
                } => {
                    volume = new_volume.clamp(0.0, 1.0);
                    update_state(
                        &state,
                        is_playing,
                        current_playback_position(decoded_position_secs, output.as_ref()),
                        duration_secs,
                        volume,
                        current_source.clone(),
                        false,
                    );
                    let _ = reply.send(Ok(()));
                }
                AudioCommand::SetEqEnabled { enabled, reply } => {
                    eq_enabled = enabled;
                    if let Some(eq) = equalizer.as_mut() {
                        eq.set_enabled(enabled);
                    }
                    update_eq_state(&eq_state, eq_enabled, eq_gains);
                    let _ = reply.send(Ok(()));
                }
                AudioCommand::SetEqGains { gains, reply } => {
                    eq_gains = gains.map(|value| value.clamp(-12.0, 12.0));
                    if let Some(eq) = equalizer.as_mut() {
                        eq.set_gains(&eq_gains);
                        eq.reset();
                    }
                    update_eq_state(&eq_state, eq_enabled, eq_gains);
                    let _ = reply.send(Ok(()));
                }
            }
        }

        if is_playing {
            if let (Some(dec), Some(out)) = (decoder.as_mut(), output.as_mut()) {
                if !pending_samples.is_empty() {
                    let written = out.producer.push_slice(&pending_samples);
                    if written > 0 {
                        pending_samples.drain(..written);
                    }
                }

                for _ in 0..24 {
                    if end_pending || !pending_samples.is_empty() {
                        break;
                    }

                    if out.producer.vacant_len() < 4096 {
                        break;
                    }

                    match dec.decode_next() {
                        Ok(Some(samples)) => {
                            let decoded_frames = samples.len() / source_channels;
                            let out_channels = out.config.channels as usize;
                            let samples = convert_channels(&samples, source_channels, out_channels);

                            if let Some(rs) = resampler.as_mut() {
                                resample_buffer.extend_from_slice(&samples);
                                let needed = rs.input_frames_needed() * out_channels;
                                while resample_buffer.len() >= needed {
                                    let chunk: Vec<f32> = resample_buffer.drain(..needed).collect();
                                    match rs.process(&chunk) {
                                        Ok(mut resampled) => {
                                            if let Some(eq) = equalizer.as_mut() {
                                                eq.process(&mut resampled);
                                            }
                                            apply_volume(&mut resampled, volume);
                                            fft_processor.push_samples(&resampled, out_channels);
                                            push_or_pend(
                                                &mut out.producer,
                                                &resampled,
                                                &mut pending_samples,
                                            );
                                        }
                                        Err(err) => {
                                            eprintln!("Resample error: {err}");
                                            break;
                                        }
                                    }

                                    if !pending_samples.is_empty() {
                                        break;
                                    }
                                }
                            } else {
                                let mut samples = samples;
                                if let Some(eq) = equalizer.as_mut() {
                                    eq.process(&mut samples);
                                }
                                apply_volume(&mut samples, volume);
                                fft_processor.push_samples(&samples, out_channels);
                                push_or_pend(&mut out.producer, &samples, &mut pending_samples);
                            }

                            decoded_position_secs +=
                                decoded_frames as f64 / source_sample_rate as f64;
                            if duration_secs > 0.0 {
                                decoded_position_secs = decoded_position_secs.min(duration_secs);
                            }
                        }
                        Ok(None) => {
                            if duration_secs <= 0.0
                                || (decoded_position_secs - duration_secs).abs() > 1.0
                            {
                                duration_secs = decoded_position_secs;
                            }
                            end_pending = true;
                            break;
                        }
                        Err(err) => {
                            eprintln!("Decode error: {err}");
                            is_playing = false;
                            if let Some(out) = output.as_ref() {
                                out.pause();
                            }
                            update_state(
                                &state,
                                false,
                                current_playback_position(decoded_position_secs, output.as_ref()),
                                duration_secs,
                                volume,
                                current_source.clone(),
                                false,
                            );
                            break;
                        }
                    }
                }
            }
        }

        if end_pending {
            let buffered_samples = output
                .as_ref()
                .map(|out| out.producer.occupied_len())
                .unwrap_or(0);

            if pending_samples.is_empty() && buffered_samples == 0 {
                end_pending = false;
                is_playing = false;
                if let Some(out) = output.as_ref() {
                    out.pause();
                }
                let final_position = duration_secs.max(decoded_position_secs);
                update_state(
                    &state,
                    false,
                    final_position,
                    duration_secs.max(final_position),
                    volume,
                    current_source.clone(),
                    true,
                );
            }
        } else if is_playing {
            update_state(
                &state,
                true,
                current_playback_position(decoded_position_secs, output.as_ref()),
                duration_secs,
                volume,
                current_source.clone(),
                false,
            );
        }

        if is_playing && last_fft_emit.elapsed() >= Duration::from_millis(33) {
            let (frequency, waveform) = fft_processor.compute();
            update_fft_state(&fft_state, frequency, waveform);
            last_fft_emit = Instant::now();
        }

        std::thread::sleep(if is_playing {
            Duration::from_millis(2)
        } else {
            Duration::from_millis(12)
        });
    }
}

fn prepare_playback(source: &str) -> Result<PreparedPlayback, String> {
    let decoder = AudioDecoder::open(source)?;
    let source_sample_rate = decoder.info.sample_rate;
    let source_channels = decoder.info.channels;
    let duration_secs = decoder.info.duration_secs;
    let requested_channels = source_channels.min(2) as u16;

    let output = AudioOutput::new(source_sample_rate, requested_channels)?;
    let output_sample_rate = output.config.sample_rate.0;
    let output_channels = output.config.channels as usize;
    let equalizer = Equalizer::new(output_sample_rate, output_channels);

    let resampler = if output_sample_rate != source_sample_rate {
        Some(AudioResampler::new(
            source_sample_rate,
            output_sample_rate,
            output_channels,
        )?)
    } else {
        None
    };

    Ok(PreparedPlayback {
        decoder,
        output,
        resampler,
        equalizer,
        source_sample_rate,
        source_channels,
        duration_secs,
    })
}

fn pause_playback(
    decoder: &mut Option<AudioDecoder>,
    output: Option<&AudioOutput>,
    resampler: &mut Option<AudioResampler>,
    equalizer: &mut Option<Equalizer>,
    resample_buffer: &mut Vec<f32>,
    pending_samples: &mut Vec<f32>,
    decoded_position_secs: &mut f64,
    duration_secs: f64,
    is_playing: &mut bool,
) -> Result<f64, String> {
    if decoder.is_none() || output.is_none() {
        return Err("Nothing to pause".to_string());
    }

    let paused_at = current_playback_position(*decoded_position_secs, output);
    if let Some(decoder) = decoder.as_mut() {
        decoder.seek(paused_at.min(duration_secs.max(paused_at)))?;
    }
    if let Some(out) = output {
        out.flush();
        out.pause();
    }

    *decoded_position_secs = paused_at;
    *is_playing = false;
    *resampler = None;
    if let Some(eq) = equalizer.as_mut() {
        eq.reset();
    }
    resample_buffer.clear();
    pending_samples.clear();
    Ok(paused_at)
}

fn seek_playback(
    decoder: Option<&mut AudioDecoder>,
    output: Option<&AudioOutput>,
    resampler: &mut Option<AudioResampler>,
    equalizer: &mut Option<Equalizer>,
    resample_buffer: &mut Vec<f32>,
    pending_samples: &mut Vec<f32>,
    decoded_position_secs: &mut f64,
    duration_secs: f64,
    position_secs: f64,
) -> Result<f64, String> {
    let decoder = decoder.ok_or_else(|| "Nothing to seek".to_string())?;
    let target = if duration_secs > 0.0 {
        position_secs.clamp(0.0, duration_secs)
    } else {
        position_secs.max(0.0)
    };

    decoder.seek(target)?;
    if let Some(out) = output {
        out.flush();
    }

    *resampler = None;
    if let Some(eq) = equalizer.as_mut() {
        eq.reset();
    }
    resample_buffer.clear();
    pending_samples.clear();
    *decoded_position_secs = target;
    Ok(target)
}

fn stop_playback(
    decoder: &mut Option<AudioDecoder>,
    output: &mut Option<AudioOutput>,
    resampler: &mut Option<AudioResampler>,
    equalizer: &mut Option<Equalizer>,
    resample_buffer: &mut Vec<f32>,
    pending_samples: &mut Vec<f32>,
    decoded_position_secs: &mut f64,
    duration_secs: &mut f64,
    is_playing: &mut bool,
    current_source: &mut Option<String>,
    end_pending: &mut bool,
) {
    *decoder = None;
    *output = None;
    *resampler = None;
    *equalizer = None;
    resample_buffer.clear();
    pending_samples.clear();
    *decoded_position_secs = 0.0;
    *duration_secs = 0.0;
    *is_playing = false;
    *current_source = None;
    *end_pending = false;
}

fn update_state(
    state: &Arc<Mutex<PlaybackState>>,
    is_playing: bool,
    position_secs: f64,
    duration_secs: f64,
    volume: f32,
    current_source: Option<String>,
    has_ended: bool,
) {
    if let Ok(mut value) = state.lock() {
        value.is_playing = is_playing;
        value.position_secs = position_secs.max(0.0);
        value.duration_secs = duration_secs.max(0.0);
        value.volume = volume.clamp(0.0, 1.0);
        value.current_source = current_source;
        value.has_ended = has_ended;
    }
}

fn update_fft_state(fft_state: &Arc<Mutex<FftSnapshot>>, frequency: Vec<u8>, waveform: Vec<u8>) {
    if let Ok(mut value) = fft_state.lock() {
        value.frequency = frequency;
        value.waveform = waveform;
    }
}

fn update_eq_state(eq_state: &Arc<Mutex<EqState>>, enabled: bool, gains: [f32; 10]) {
    if let Ok(mut value) = eq_state.lock() {
        value.enabled = enabled;
        value.gains = gains;
    }
}

fn current_playback_position(position_secs: f64, output: Option<&AudioOutput>) -> f64 {
    if let Some(out) = output {
        let buffered_samples = out.producer.occupied_len();
        let out_rate = out.config.sample_rate.0 as f64;
        let out_channels = out.config.channels as f64;
        if out_rate > 0.0 && out_channels > 0.0 {
            let buffered_secs = buffered_samples as f64 / (out_rate * out_channels);
            return (position_secs - buffered_secs).max(0.0);
        }
    }
    position_secs.max(0.0)
}

fn apply_volume(samples: &mut [f32], volume: f32) {
    for sample in samples.iter_mut() {
        *sample = (*sample * volume).clamp(-1.0, 1.0);
    }
}

fn push_or_pend(producer: &mut HeapProd<f32>, samples: &[f32], pending: &mut Vec<f32>) {
    let written = producer.push_slice(samples);
    if written < samples.len() {
        pending.extend_from_slice(&samples[written..]);
    }
}

fn convert_channels(samples: &[f32], from_channels: usize, to_channels: usize) -> Vec<f32> {
    if from_channels == to_channels {
        return samples.to_vec();
    }

    let frames = samples.len() / from_channels;
    let mut output = Vec::with_capacity(frames * to_channels);

    if from_channels == 1 && to_channels == 2 {
        for frame in 0..frames {
            let sample = samples[frame];
            output.push(sample);
            output.push(sample);
        }
        return output;
    }

    if from_channels == 2 && to_channels == 1 {
        for frame in 0..frames {
            let left = samples[frame * 2];
            let right = samples[frame * 2 + 1];
            output.push((left + right) * 0.5);
        }
        return output;
    }

    for frame in 0..frames {
        for channel in 0..to_channels {
            let source_channel = channel.min(from_channels - 1);
            output.push(samples[frame * from_channels + source_channel]);
        }
    }

    output
}
