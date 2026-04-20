use rustfft::{num_complex::Complex, FftPlanner};

const FFT_SIZE: usize = 2048;
const FREQ_BINS: usize = 64;
const WAVEFORM_POINTS: usize = 128;

/// FFT processor that maintains a mono sample ring buffer,
/// computes frequency spectrum and waveform data.
pub struct FftProcessor {
    buffer: Vec<f32>,     // mono sample ring buffer
    write_pos: usize,
    planner: FftPlanner<f32>,
    window: Vec<f32>,     // Hann window
    enabled: bool,
}

impl FftProcessor {
    pub fn new() -> Self {
        // Precompute Hann window
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
            })
            .collect();

        Self {
            buffer: vec![0.0; FFT_SIZE],
            write_pos: 0,
            planner: FftPlanner::new(),
            window,
            enabled: false,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.buffer.fill(0.0);
            self.write_pos = 0;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Feed interleaved multi-channel samples; internally mixes to mono.
    pub fn push_samples(&mut self, samples: &[f32], channels: usize) {
        if !self.enabled || channels == 0 {
            return;
        }

        let frames = samples.len() / channels;
        for frame in 0..frames {
            let mut mono = 0.0f32;
            for ch in 0..channels {
                mono += samples[frame * channels + ch];
            }
            mono /= channels as f32;

            self.buffer[self.write_pos] = mono;
            self.write_pos = (self.write_pos + 1) % FFT_SIZE;
        }
    }

    /// Compute FFT and return (frequency_bins[64], waveform_points[128]) as u8 arrays.
    pub fn compute(&mut self) -> (Vec<u8>, Vec<u8>) {
        if !self.enabled {
            return (vec![0u8; FREQ_BINS], vec![128u8; WAVEFORM_POINTS]);
        }

        // Build windowed complex input (read from ring buffer in order)
        let fft = self.planner.plan_fft_forward(FFT_SIZE);
        let mut input: Vec<Complex<f32>> = (0..FFT_SIZE)
            .map(|i| {
                let idx = (self.write_pos + i) % FFT_SIZE;
                Complex::new(self.buffer[idx] * self.window[i], 0.0)
            })
            .collect();

        fft.process(&mut input);

        // Compute magnitudes (only first half = Nyquist)
        let half = FFT_SIZE / 2;
        let magnitudes: Vec<f32> = input[..half]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt() / (FFT_SIZE as f32))
            .collect();

        // Logarithmic binning into FREQ_BINS
        let frequency = log_bin_magnitudes(&magnitudes, FREQ_BINS);

        // Waveform: sample WAVEFORM_POINTS points from the ring buffer
        let waveform = sample_waveform(&self.buffer, self.write_pos, WAVEFORM_POINTS);

        (frequency, waveform)
    }
}

/// Bin magnitudes into `num_bins` frequency bands using logarithmic spacing.
fn log_bin_magnitudes(magnitudes: &[f32], num_bins: usize) -> Vec<u8> {
    let len = magnitudes.len();
    let mut bins = vec![0u8; num_bins];

    for i in 0..num_bins {
        // Logarithmic frequency mapping
        let lo = ((i as f64 / num_bins as f64).powi(2) * len as f64) as usize;
        let hi = (((i + 1) as f64 / num_bins as f64).powi(2) * len as f64) as usize;
        let lo = lo.min(len - 1);
        let hi = hi.max(lo + 1).min(len);

        let mut max_val = 0.0f32;
        for j in lo..hi {
            if magnitudes[j] > max_val {
                max_val = magnitudes[j];
            }
        }

        // Scale to 0-255 with some amplification
        let db = 20.0 * (max_val.max(1e-10)).log10();
        // Map roughly -60dB..0dB to 0..255
        let normalized = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        bins[i] = (normalized * 255.0) as u8;
    }

    bins
}

/// Sample waveform points from ring buffer, mapping float [-1, 1] to u8 [0, 255].
fn sample_waveform(buffer: &[f32], write_pos: usize, num_points: usize) -> Vec<u8> {
    let len = buffer.len();
    let mut points = vec![128u8; num_points];

    for i in 0..num_points {
        let idx = (write_pos + i * len / num_points) % len;
        // Map -1..1 to 0..255 with 128 as center
        let val = ((buffer[idx] * 127.0) + 128.0).clamp(0.0, 255.0);
        points[i] = val as u8;
    }

    points
}
