use rubato::{FftFixedInOut, Resampler};

/// Resamples interleaved f32 audio from one sample rate to another.
pub struct AudioResampler {
    resampler: FftFixedInOut<f32>,
    channels: usize,
    input_frames_needed: usize,
}

impl AudioResampler {
    pub fn new(from_rate: u32, to_rate: u32, channels: usize) -> Result<Self, String> {
        if from_rate == to_rate {
            return Err("No resampling needed".to_string());
        }

        let chunk_size = 1024;
        let resampler = FftFixedInOut::<f32>::new(
            from_rate as usize,
            to_rate as usize,
            chunk_size,
            channels,
        )
        .map_err(|e| format!("Failed to create resampler: {}", e))?;

        let input_frames_needed = resampler.input_frames_next();

        Ok(Self {
            resampler,
            channels,
            input_frames_needed,
        })
    }

    /// Returns the number of input frames needed for the next processing call.
    pub fn input_frames_needed(&self) -> usize {
        self.input_frames_needed
    }

    /// Process interleaved samples. Returns resampled interleaved samples.
    /// Input must contain exactly `input_frames_needed() * channels` samples.
    pub fn process(&mut self, interleaved: &[f32]) -> Result<Vec<f32>, String> {
        let frames = interleaved.len() / self.channels;
        if frames != self.input_frames_needed {
            return Err(format!(
                "Expected {} frames, got {}",
                self.input_frames_needed, frames
            ));
        }

        // De-interleave into per-channel buffers
        let mut input_channels: Vec<Vec<f32>> = vec![vec![0.0; frames]; self.channels];
        for frame in 0..frames {
            for ch in 0..self.channels {
                input_channels[ch][frame] = interleaved[frame * self.channels + ch];
            }
        }

        let input_refs: Vec<&[f32]> = input_channels.iter().map(|c| c.as_slice()).collect();
        let output_channels = self
            .resampler
            .process(&input_refs, None)
            .map_err(|e| format!("Resample error: {}", e))?;

        self.input_frames_needed = self.resampler.input_frames_next();

        // Re-interleave
        let output_frames = output_channels[0].len();
        let mut out = Vec::with_capacity(output_frames * self.channels);
        for frame in 0..output_frames {
            for ch in 0..self.channels {
                out.push(output_channels[ch][frame]);
            }
        }

        Ok(out)
    }
}
