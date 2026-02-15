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
}

impl AudioOutput {
    /// Create a new audio output with a ring buffer.
    /// The ring buffer size is ~1 second of audio at the given sample rate and channels.
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

        // Ring buffer: ~2 seconds for comfortable headroom
        let buf_size = (actual_rate as usize) * (config.channels as usize) * 2;
        let rb = HeapRb::<f32>::new(buf_size.max(4096));
        let (producer, consumer) = rb.split();

        let playing = Arc::new(AtomicBool::new(true));
        let playing_clone = playing.clone();

        let stream = build_output_stream(&device, &config, consumer, playing_clone)?;
        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        Ok(Self {
            _stream: stream,
            producer,
            config,
            playing,
        })
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.playing.store(true, Ordering::Relaxed);
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: HeapCons<f32>,
    playing: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if !playing.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                let read = consumer.pop_slice(data);
                // Fill remaining with silence
                data[read..].fill(0.0);
            },
            |err| {
                eprintln!("Audio output error: {}", err);
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}
