/// 10-band Biquad EQ filter.
///
/// Band 0 (32 Hz): lowshelf
/// Bands 1-8 (64–8000 Hz): peaking, Q = 1.4
/// Band 9 (16000 Hz): highshelf
///
/// Each channel gets independent filter state (stereo = 20 instances).

const EQ_FREQUENCIES: [f32; 10] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

#[derive(Clone)]
struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

#[derive(Clone)]
struct BiquadState {
    z1: f64,
    z2: f64,
}

impl BiquadState {
    fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    fn process(&mut self, coeffs: &BiquadCoeffs, x: f64) -> f64 {
        // Direct Form II Transposed
        let y = coeffs.b0 * x + self.z1;
        self.z1 = coeffs.b1 * x - coeffs.a1 * y + self.z2;
        self.z2 = coeffs.b2 * x - coeffs.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FilterType {
    LowShelf,
    Peaking,
    HighShelf,
}

fn compute_coeffs(filter_type: FilterType, freq: f64, gain_db: f64, q: f64, sample_rate: f64) -> BiquadCoeffs {
    let a = 10.0_f64.powf(gain_db / 40.0); // sqrt of linear gain
    let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();

    let (b0, b1, b2, a0, a1, a2);

    match filter_type {
        FilterType::LowShelf => {
            let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).sqrt();
            let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

            b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
            b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
            b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
            a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
            a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
            a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;
        }
        FilterType::Peaking => {
            let alpha = sin_w0 / (2.0 * q);

            b0 = 1.0 + alpha * a;
            b1 = -2.0 * cos_w0;
            b2 = 1.0 - alpha * a;
            a0 = 1.0 + alpha / a;
            a1 = -2.0 * cos_w0;
            a2 = 1.0 - alpha / a;
        }
        FilterType::HighShelf => {
            let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / q - 1.0) + 2.0).sqrt();
            let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

            b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
            b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
            b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
            a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
            a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
            a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;
        }
    }

    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// 10-band parametric EQ that processes interleaved f32 audio in-place.
pub struct Equalizer {
    coeffs: Vec<BiquadCoeffs>,            // 10 bands
    states: Vec<Vec<BiquadState>>,        // 10 bands × N channels
    gains: [f32; 10],
    enabled: bool,
    sample_rate: f64,
    channels: usize,
}

impl Equalizer {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let gains = [0.0f32; 10];
        let sr = sample_rate as f64;

        let mut coeffs = Vec::with_capacity(10);
        let mut states = Vec::with_capacity(10);

        for (i, &freq) in EQ_FREQUENCIES.iter().enumerate() {
            let ft = if i == 0 {
                FilterType::LowShelf
            } else if i == 9 {
                FilterType::HighShelf
            } else {
                FilterType::Peaking
            };
            let q = if ft == FilterType::Peaking { 1.4 } else { 0.707 };
            coeffs.push(compute_coeffs(ft, freq as f64, 0.0, q, sr));
            states.push(vec![BiquadState::new(); channels]);
        }

        Self {
            coeffs,
            states,
            gains,
            enabled: true,
            sample_rate: sr,
            channels,
        }
    }

    pub fn set_gains(&mut self, gains: &[f32; 10]) {
        self.gains = *gains;
        self.recompute_coeffs();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn gains(&self) -> [f32; 10] {
        self.gains
    }

    pub fn reset(&mut self) {
        for band_states in &mut self.states {
            for s in band_states.iter_mut() {
                s.reset();
            }
        }
    }

    /// Process interleaved f32 samples in-place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        let channels = self.channels;
        let frames = samples.len() / channels;

        for frame in 0..frames {
            for ch in 0..channels {
                let idx = frame * channels + ch;
                let mut sample = samples[idx] as f64;

                for band in 0..10 {
                    sample = self.states[band][ch].process(&self.coeffs[band], sample);
                }

                samples[idx] = (sample as f32).clamp(-1.0, 1.0);
            }
        }
    }

    fn recompute_coeffs(&mut self) {
        for (i, &freq) in EQ_FREQUENCIES.iter().enumerate() {
            let ft = if i == 0 {
                FilterType::LowShelf
            } else if i == 9 {
                FilterType::HighShelf
            } else {
                FilterType::Peaking
            };
            let q = if ft == FilterType::Peaking { 1.4 } else { 0.707 };
            self.coeffs[i] = compute_coeffs(ft, freq as f64, self.gains[i] as f64, q, self.sample_rate);
        }
    }
}
