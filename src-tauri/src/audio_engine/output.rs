use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct AudioOutput {
    _stream: Stream,
    pub producer: HeapProd<f32>,
    pub config: StreamConfig,
    playing: Arc<AtomicBool>,
    flushing: Arc<AtomicBool>,
    stream_error: Arc<AtomicBool>,
}

impl AudioOutput {
    /// Create a new audio output with a ring buffer.
    /// The ring buffer size is ~250ms of audio at the given sample rate and channels
    /// to keep volume/mute response snappy while still absorbing small jitter.
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No audio output device found")?;

        let supported_config = device
            .supported_output_configs()
            .map_err(|e| format!("Failed to query output configs: {}", e))?
            .find(|c| {
                c.channels() == channels
                    && c.min_sample_rate().0 <= sample_rate
                    && c.max_sample_rate().0 >= sample_rate
                    && c.sample_format() == SampleFormat::F32
            })
            .or_else(|| {
                // Fallback: any config with F32
                device
                    .supported_output_configs()
                    .ok()?
                    .find(|c| c.sample_format() == SampleFormat::F32)
            })
            .ok_or("No suitable audio output configuration found")?;

        // Clamp sample rate to the supported range of the chosen config
        let actual_rate = sample_rate
            .max(supported_config.min_sample_rate().0)
            .min(supported_config.max_sample_rate().0);

        let config = supported_config
            .with_sample_rate(cpal::SampleRate(actual_rate))
            .config();

        // Ring buffer: ~250ms for lower control latency (volume/mute), still enough headroom
        let frames_250ms = (actual_rate as usize) / 4;
        let buf_size = frames_250ms * (config.channels as usize);
        let rb = HeapRb::<f32>::new(buf_size.max(4096));
        let (producer, consumer) = rb.split();

        let playing = Arc::new(AtomicBool::new(true));
        let playing_clone = playing.clone();
        let flushing = Arc::new(AtomicBool::new(false));
        let flushing_clone = flushing.clone();
        let stream_error = Arc::new(AtomicBool::new(false));
        let stream_error_clone = stream_error.clone();

        let stream = build_output_stream(
            &device,
            &config,
            consumer,
            playing_clone,
            flushing_clone,
            stream_error_clone,
        )?;
        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        Ok(Self {
            _stream: stream,
            producer,
            config,
            playing,
            flushing,
            stream_error,
        })
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.playing.store(true, Ordering::Relaxed);
    }

    /// Signal the output callback to discard all buffered audio.
    pub fn flush(&self) {
        self.flushing.store(true, Ordering::Relaxed);
    }

    pub fn has_stream_error(&self) -> bool {
        self.stream_error.load(Ordering::Relaxed)
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: HeapCons<f32>,
    playing: Arc<AtomicBool>,
    flushing: Arc<AtomicBool>,
    stream_error: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let mut flush_buf = vec![0.0f32; 4096];
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // On flush: drain all buffered data and output silence
                if flushing.load(Ordering::Relaxed) {
                    while consumer.pop_slice(&mut flush_buf) > 0 {}
                    flushing.store(false, Ordering::Relaxed);
                    data.fill(0.0);
                    return;
                }
                if !playing.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                let read = consumer.pop_slice(data);
                if read < data.len() {
                    // Underrun: fade the last valid sample to silence
                    // instead of a hard cut to 0.0 which causes clicks.
                    let last_sample = if read > 0 { data[read - 1] } else { 0.0 };
                    let remaining = data.len() - read;
                    let fade_len = remaining.min(64); // ~1.5ms at 44100Hz
                    for i in 0..fade_len {
                        let t = (i + 1) as f32 / (fade_len + 1) as f32;
                        data[read + i] = last_sample * (1.0 - t);
                    }
                    data[read + fade_len..].fill(0.0);
                }
            },
            move |err| {
                eprintln!("Audio output error: {}", err);
                stream_error.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}
