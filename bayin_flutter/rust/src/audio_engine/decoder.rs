use std::fs::File;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use super::http_source::HttpStreamSource;

pub struct DecodedInfo {
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_secs: f64,
}

pub struct AudioDecoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    pub info: DecodedInfo,
}

impl AudioDecoder {
    /// Open a local file or HTTP URL for decoding.
    pub fn open(source: &str) -> Result<Self, String> {
        let mss = if source.starts_with("http://") || source.starts_with("https://") {
            // HTTP source: stream via sequential reads (not full download)
            let http_source = HttpStreamSource::open(source)?;
            MediaSourceStream::new(Box::new(http_source), Default::default())
        } else {
            // Local file
            let file =
                File::open(source).map_err(|e| format!("Failed to open file '{}': {}", source, e))?;
            MediaSourceStream::new(Box::new(file), Default::default())
        };

        let mut hint = Hint::new();
        // Try to extract extension from source path
        if let Some(ext) = std::path::Path::new(source)
            .extension()
            .and_then(|e| e.to_str())
        {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| format!("Failed to probe audio format: {}", e))?;

        let format_reader = probed.format;

        // Find the first audio track
        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No supported audio track found")?;

        let track_id = track.id;
        let codec_params = &track.codec_params;

        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params
            .channels
            .map(|c| c.count())
            .unwrap_or(2);

        // Calculate duration
        let duration_secs = codec_params
            .n_frames
            .filter(|&n| n > 0)
            .map(|n| {
                if let Some(tb) = codec_params.time_base {
                    let t = tb.calc_time(n);
                    t.seconds as f64 + t.frac
                } else {
                    n as f64 / sample_rate as f64
                }
            })
            .unwrap_or(0.0);

        let decoder = symphonia::default::get_codecs()
            .make(codec_params, &decoder_opts)
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            info: DecodedInfo {
                sample_rate,
                channels,
                duration_secs,
            },
        })
    }

    /// Decode the next packet into interleaved f32 samples.
    /// Returns None at end of stream.
    pub fn decode_next(&mut self) -> Result<Option<Vec<f32>>, String> {
        loop {
            let packet = match self.format_reader.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(format!("Decode error: {}", e)),
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let samples = audio_buf_to_f32(&decoded, self.info.channels);
                    return Ok(Some(samples));
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(format!("Decode error: {}", e)),
            }
        }
    }

    /// Seek to a position in seconds.
    pub fn seek(&mut self, position_secs: f64) -> Result<(), String> {
        let clamped = if self.info.duration_secs > 0.0 {
            position_secs.clamp(0.0, (self.info.duration_secs - 0.1).max(0.0))
        } else {
            position_secs.max(0.0)
        };
        let seek_to = SeekTo::Time {
            time: Time::from(clamped),
            track_id: Some(self.track_id),
        };
        self.format_reader
            .seek(SeekMode::Accurate, seek_to)
            .map_err(|e| format!("Seek failed: {}", e))?;
        self.decoder.reset();
        Ok(())
    }
}

/// Convert any symphonia AudioBufferRef to interleaved f32 samples.
fn audio_buf_to_f32(buf: &AudioBufferRef, channels: usize) -> Vec<f32> {
    let frames = buf.frames();
    let mut out = Vec::with_capacity(frames * channels);

    match buf {
        AudioBufferRef::U8(b) => {
            for frame in 0..frames {
                for ch in 0..channels.min(b.spec().channels.count()) {
                    out.push((b.chan(ch)[frame] as f32 - 128.0) / 128.0);
                }
            }
        }
        AudioBufferRef::S16(b) => {
            for frame in 0..frames {
                for ch in 0..channels.min(b.spec().channels.count()) {
                    out.push(b.chan(ch)[frame] as f32 / 32768.0);
                }
            }
        }
        AudioBufferRef::S24(b) => {
            for frame in 0..frames {
                for ch in 0..channels.min(b.spec().channels.count()) {
                    out.push(b.chan(ch)[frame].0 as f32 / 8388608.0);
                }
            }
        }
        AudioBufferRef::S32(b) => {
            for frame in 0..frames {
                for ch in 0..channels.min(b.spec().channels.count()) {
                    out.push(b.chan(ch)[frame] as f32 / 2147483648.0);
                }
            }
        }
        AudioBufferRef::F32(b) => {
            for frame in 0..frames {
                for ch in 0..channels.min(b.spec().channels.count()) {
                    out.push(b.chan(ch)[frame]);
                }
            }
        }
        AudioBufferRef::F64(b) => {
            for frame in 0..frames {
                for ch in 0..channels.min(b.spec().channels.count()) {
                    out.push(b.chan(ch)[frame] as f32);
                }
            }
        }
        _ => {
            // Unsigned 16/24/32 and signed 8 — rare formats, treat as silence
            eprintln!("Unsupported audio sample format, skipping packet");
        }
    }

    out
}
