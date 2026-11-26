use crate::state_variable_filter::{Response, StateVariableFilter};
use kahuna_core::ModuleEnv;
use kahuna_unit::kahuna_module;

/// Two-pole filter whose type follows what is connected: only `low_pass_cutoff` makes a low pass,
/// only `high_pass_cutoff` a high pass, both a band pass with the two cutoffs as its edges, and
/// neither passes the signal through unchanged.
///
/// The band pass is a high pass followed by a low pass: a flat window with independent edges, not
/// a resonant peak around one center frequency; for that, use `band-pass`. `resonance` applies to each edge.
#[kahuna_module(id = "filter")]
pub struct Filter {
    high_pass: StateVariableFilter,
    low_pass: StateVariableFilter,

    #[input]
    input: Audio,

    #[input]
    high_pass_cutoff: Parameter<Option<f32>>,

    #[input]
    low_pass_cutoff: Parameter<Option<f32>>,

    #[input(default = 0.3)]
    resonance: Control,

    #[output]
    out: Audio,
}

impl Filter {
    pub fn init(module_env: &ModuleEnv) -> Self {
        Self {
            high_pass: StateVariableFilter::new(module_env.sample_rate),
            low_pass: StateVariableFilter::new(module_env.sample_rate),
        }
    }

    pub fn process<'a>(&mut self, inputs: &FilterInputs<'a>, outputs: &mut FilterOutputs<'a>) {
        outputs.out.copy_from_slice(inputs.input);

        if let Some(cutoff) = inputs.high_pass_cutoff {
            self.high_pass.process_in_place(Response::HighPass, cutoff, inputs.resonance, outputs.out);
        }
        if let Some(cutoff) = inputs.low_pass_cutoff {
            self.low_pass.process_in_place(Response::LowPass, cutoff, inputs.resonance, outputs.out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kahuna_core::{KahunaModule, PortDirection, StreamKind};
    use std::f32::consts::PI;

    const SAMPLE_RATE: u32 = 48000;
    const FRAMES: usize = 4096;

    fn run(input: &[f32], high_pass_cutoff: Option<f32>, low_pass_cutoff: Option<f32>) -> Vec<f32> {
        let env = ModuleEnv { sample_rate: SAMPLE_RATE, max_buffer_size: FRAMES };
        let mut filter = Filter::new(&env);

        let mut out = vec![0.0; input.len()];
        let inputs = FilterInputs::new(input, high_pass_cutoff, low_pass_cutoff, None);
        let mut outputs = FilterOutputs::new(&mut out);
        filter.process(&inputs, &mut outputs);

        out
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
    fn test_id() {
        assert_eq!(Filter::ID, "filter");
    }

    #[test]
    fn test_ports_reflection() {
        let names: Vec<_> = Filter::PORTS.iter().map(|port| port.name).collect();
        assert_eq!(names, ["input", "high_pass_cutoff", "low_pass_cutoff", "resonance", "out"]);

        let input = &Filter::PORTS[0];
        assert_eq!(input.stream_kind, StreamKind::Signal);
        assert_eq!(input.required, Some(true));

        for cutoff in &Filter::PORTS[1..3] {
            assert_eq!(cutoff.direction, PortDirection::Input);
            assert_eq!(cutoff.stream_kind, StreamKind::Parameter);
            assert_eq!(cutoff.required, Some(false));
            assert!(cutoff.value_type.matches(&kahuna_core::ValueType::of::<f32>()));
        }

        assert_eq!(Filter::PORTS[3].required, Some(false));
        assert_eq!(Filter::PORTS[4].direction, PortDirection::Output);
    }

    #[test]
    fn test_without_cutoffs_the_signal_passes_unchanged() {
        let input = sine(440.0);

        assert_eq!(run(&input, None, None), input);
    }

    #[test]
    fn test_low_pass_only() {
        let high_tone = run(&sine(10_000.0), None, Some(500.0));
        let dc = run(&vec![1.0; FRAMES], None, Some(500.0));

        assert!(settled_peak(&high_tone) < 0.01);
        assert!((dc[FRAMES - 1] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_high_pass_only() {
        let high_tone = run(&sine(10_000.0), Some(500.0), None);
        let dc = run(&vec![1.0; FRAMES], Some(500.0), None);

        assert!(settled_peak(&high_tone) > 0.9);
        assert!(dc[FRAMES - 1].abs() < 1e-3);
    }

    #[test]
    fn test_both_cutoffs_make_a_band_pass() {
        let (high_pass_cutoff, low_pass_cutoff) = (500.0, 5000.0);

        let inside = run(&sine(1500.0), Some(high_pass_cutoff), Some(low_pass_cutoff));
        let below = run(&sine(50.0), Some(high_pass_cutoff), Some(low_pass_cutoff));
        let above = run(&sine(20_000.0), Some(high_pass_cutoff), Some(low_pass_cutoff));

        assert!(settled_peak(&inside) > 0.8);
        assert!(settled_peak(&below) < 0.05);
        assert!(settled_peak(&above) < 0.05);
    }
}
