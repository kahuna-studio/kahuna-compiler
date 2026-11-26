use std::f32::consts::PI;

const MIN_CUTOFF_HZ: f32 = 20.0;
/// tan() grows without bound towards Nyquist (0.5 * sample rate), so stay clear of it.
const MAX_CUTOFF_RATIO: f32 = 0.45;
/// Damping at resonance 0 (Q = 0.5, no peak) and at resonance 1 (Q = 50, rings for a long time).
const MAX_DAMPING: f32 = 2.0;
const MIN_DAMPING: f32 = 0.02;
const MIN_BANDWIDTH_OCTAVES: f32 = 0.05;
const MAX_BANDWIDTH_OCTAVES: f32 = 4.0;
const DENORMAL_THRESHOLD: f32 = 1e-20;

/// Which of the filter's three outputs a block is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    LowPass,
    BandPass,
    HighPass,
}

/// Two-pole (12 dB/octave) state variable filter, in the topology-preserving-transform form of
/// Zavalishin and Simper. A building block for filter units; it is not a unit itself.
///
/// `resonance` runs from 0 to 1; 0.3 is close to a Butterworth response (flat passband, no peak).
/// Cutoff and resonance changes are ramped across each block to avoid zipper noise.
///
/// One instance holds the state of one filter stage: a unit that cascades stages owns one instance
/// per stage, and must always feed an instance the same stream.
pub struct StateVariableFilter {
    sample_rate: f32,
    /// What the previous block ended with; `None` until the first block.
    coefficients: Option<Coefficients>,
    first_integrator: f32,
    second_integrator: f32,
}

impl StateVariableFilter {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            coefficients: None,
            first_integrator: 0.0,
            second_integrator: 0.0,
        }
    }

    /// Filters `buffer` in place, with the sharpness of the filter set by `resonance`.
    /// The band pass has unity gain at the cutoff, whatever the resonance.
    pub fn process_in_place(&mut self, response: Response, cutoff_hz: f32, resonance: f32, buffer: &mut [f32]) {
        let target = Coefficients::from_resonance(cutoff_hz, resonance, self.sample_rate);
        self.process_towards(response, target, buffer);
    }

    /// Band-passes `buffer` in place around `center_hz`. The band is `bandwidth_octaves` wide between
    /// its -3 dB edges, which lie `bandwidth_octaves / 2` octaves below and above the center,
    /// so the width in octaves stays the same wherever the center moves.
    pub fn process_band_pass_in_place(&mut self, center_hz: f32, bandwidth_octaves: f32, buffer: &mut [f32]) {
        let target = Coefficients::from_bandwidth(center_hz, bandwidth_octaves, self.sample_rate);
        self.process_towards(Response::BandPass, target, buffer);
    }

    fn process_towards(&mut self, response: Response, target: Coefficients, buffer: &mut [f32]) {
        let start = self.coefficients.unwrap_or(target);
        let progress_per_frame = 1.0 / buffer.len().max(1) as f32;

        for (frame, sample) in buffer.iter_mut().enumerate() {
            let progress = (frame + 1) as f32 * progress_per_frame;
            *sample = self.tick(response, start.lerp(target, progress), *sample);
        }

        self.coefficients = Some(target);
        self.flush_denormals();
    }

    fn tick(&mut self, response: Response, coefficients: Coefficients, input: f32) -> f32 {
        let Coefficients { g, k } = coefficients;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.second_integrator;
        let band = a1 * self.first_integrator + a2 * v3;
        let low = self.second_integrator + a2 * self.first_integrator + a3 * v3;

        self.first_integrator = 2.0 * band - self.first_integrator;
        self.second_integrator = 2.0 * low - self.second_integrator;

        match response {
            Response::LowPass => low,
            Response::BandPass => k * band,
            Response::HighPass => input - k * band - low,
        }
    }

    /// Decaying integrators end up in the denormal range, where some CPUs get very slow.
    fn flush_denormals(&mut self) {
        for state in [&mut self.first_integrator, &mut self.second_integrator] {
            if state.abs() < DENORMAL_THRESHOLD {
                *state = 0.0;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Coefficients {
    /// Frequency warping: tan(pi * cutoff / sample rate).
    g: f32,
    /// Damping, the inverse of Q.
    k: f32,
}

impl Coefficients {
    fn from_resonance(cutoff_hz: f32, resonance: f32, sample_rate: f32) -> Self {
        // max/min instead of clamp: they swallow NaN, which would otherwise poison the filter state for good.
        let resonance = resonance.max(0.0).min(1.0);

        Self {
            g: Self::warped_frequency(cutoff_hz, sample_rate),
            k: MAX_DAMPING - (MAX_DAMPING - MIN_DAMPING) * resonance,
        }
    }

    /// A band `n` octaves wide has Q = sqrt(2^n) / (2^n - 1), so the damping 1/Q is 2^(n/2) - 2^(-n/2).
    fn from_bandwidth(center_hz: f32, bandwidth_octaves: f32, sample_rate: f32) -> Self {
        let octaves = bandwidth_octaves.max(MIN_BANDWIDTH_OCTAVES).min(MAX_BANDWIDTH_OCTAVES);
        let half_width = (octaves / 2.0).exp2();

        Self {
            g: Self::warped_frequency(center_hz, sample_rate),
            k: half_width - 1.0 / half_width,
        }
    }

    fn warped_frequency(frequency_hz: f32, sample_rate: f32) -> f32 {
        let frequency = frequency_hz.max(MIN_CUTOFF_HZ).min(sample_rate * MAX_CUTOFF_RATIO);
        (PI * frequency / sample_rate).tan()
    }

    fn lerp(self, other: Self, progress: f32) -> Self {
        Self {
            g: self.g + (other.g - self.g) * progress,
            k: self.k + (other.k - self.k) * progress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 48000;
    const FRAMES: usize = 4096;
    const BUTTERWORTH_RESONANCE: f32 = 0.3;

    fn filtered(response: Response, cutoff_hz: f32, input: &[f32]) -> Vec<f32> {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        let mut buffer = input.to_vec();
        filter.process_in_place(response, cutoff_hz, BUTTERWORTH_RESONANCE, &mut buffer);
        buffer
    }

    fn sine(frequency: f32) -> Vec<f32> {
        (0..FRAMES)
            .map(|n| (2.0 * PI * frequency * n as f32 / SAMPLE_RATE as f32).sin())
            .collect()
    }

    /// The peak of the second half, after the filter has settled.
    fn settled_peak(samples: &[f32]) -> f32 {
        samples[FRAMES / 2..].iter().fold(0.0_f32, |peak, x| peak.max(x.abs()))
    }

    #[test]
    fn low_pass_passes_dc_and_attenuates_a_tone_above_the_cutoff() {
        let dc = filtered(Response::LowPass, 1000.0, &vec![1.0; FRAMES]);
        let tone = filtered(Response::LowPass, 500.0, &sine(10_000.0));

        assert!((dc[FRAMES - 1] - 1.0).abs() < 1e-3);
        assert!(settled_peak(&tone) < 0.01);
    }

    #[test]
    fn high_pass_removes_dc_and_keeps_a_tone_above_the_cutoff() {
        let dc = filtered(Response::HighPass, 1000.0, &vec![1.0; FRAMES]);
        let tone = filtered(Response::HighPass, 500.0, &sine(10_000.0));

        assert!(dc[FRAMES - 1].abs() < 1e-3);
        assert!(settled_peak(&tone) > 0.9);
    }

    #[test]
    fn band_pass_has_unity_gain_at_the_cutoff_and_blocks_dc() {
        let at_cutoff = filtered(Response::BandPass, 1000.0, &sine(1000.0));
        let dc = filtered(Response::BandPass, 1000.0, &vec![1.0; FRAMES]);

        assert!((settled_peak(&at_cutoff) - 1.0).abs() < 0.05);
        assert!(dc[FRAMES - 1].abs() < 1e-3);
    }

    fn band_passed(center_hz: f32, bandwidth_octaves: f32, input: &[f32]) -> Vec<f32> {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        let mut buffer = input.to_vec();
        filter.process_band_pass_in_place(center_hz, bandwidth_octaves, &mut buffer);
        buffer
    }

    #[test]
    fn bandwidth_band_pass_has_its_edges_half_the_bandwidth_from_the_center() {
        let minus_three_db = 1.0 / 2.0_f32.sqrt();

        for (center, bandwidth) in [(500.0_f32, 1.0_f32), (2000.0, 1.0), (1000.0, 0.5), (1000.0, 2.0)] {
            let half_width = (bandwidth / 2.0).exp2();
            let at_center = settled_peak(&band_passed(center, bandwidth, &sine(center)));
            let at_lower_edge = settled_peak(&band_passed(center, bandwidth, &sine(center / half_width)));
            let at_upper_edge = settled_peak(&band_passed(center, bandwidth, &sine(center * half_width)));

            assert!((at_center - 1.0).abs() < 0.05, "center {center}, bandwidth {bandwidth}: {at_center}");
            assert!((at_lower_edge - minus_three_db).abs() < 0.06, "lower edge {center}/{bandwidth}: {at_lower_edge}");
            assert!((at_upper_edge - minus_three_db).abs() < 0.06, "upper edge {center}/{bandwidth}: {at_upper_edge}");
        }
    }

    #[test]
    fn narrower_bandwidth_lets_less_pass_next_to_the_center() {
        let next_to_center = sine(1500.0);
        let wide = settled_peak(&band_passed(1000.0, 2.0, &next_to_center));
        let narrow = settled_peak(&band_passed(1000.0, 0.25, &next_to_center));

        assert!(narrow < wide);
    }

    #[test]
    fn extreme_bandwidths_stay_finite() {
        for bandwidth in [-1.0, 0.0, 1.0e9, f32::NAN] {
            let output = band_passed(1000.0, bandwidth, &sine(440.0));

            assert!(output.iter().all(|sample| sample.is_finite()), "bandwidth {bandwidth}");
        }
    }

    #[test]
    fn silence_stays_silent() {
        let output = filtered(Response::LowPass, 1000.0, &vec![0.0; FRAMES]);

        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn state_carries_over_between_blocks() {
        let input = sine(300.0);
        let whole = filtered(Response::LowPass, 1000.0, &input);

        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        let mut halves = input.clone();
        let (first, second) = halves.split_at_mut(FRAMES / 2);
        filter.process_in_place(Response::LowPass, 1000.0, BUTTERWORTH_RESONANCE, first);
        filter.process_in_place(Response::LowPass, 1000.0, BUTTERWORTH_RESONANCE, second);

        // Only the first block starts from the target coefficients; both are constant here.
        assert!((whole[FRAMES - 1] - halves[FRAMES - 1]).abs() < 1e-4);
    }

    #[test]
    fn extreme_and_invalid_settings_stay_finite() {
        for (cutoff, resonance) in [(0.0, -1.0), (1.0e9, 5.0), (f32::NAN, f32::NAN)] {
            let mut filter = StateVariableFilter::new(SAMPLE_RATE);
            let mut buffer = sine(440.0);
            filter.process_in_place(Response::BandPass, cutoff, resonance, &mut buffer);

            assert!(buffer.iter().all(|sample| sample.is_finite()), "cutoff {cutoff}, resonance {resonance}");
        }
    }
}
