use crate::state_variable_filter::StateVariableFilter;
use kahuna_core::ModuleEnv;
use kahuna_unit::kahuna_module;

/// Band pass with a resonant peak around one center frequency, a "wah" or formant filter.
/// `bandwidth` is the width of the band in octaves (between its -3 dB edges) and stays the same
/// width in the ear wherever `center` is swept: 1.0 is an octave, 0.5 half an octave.
///
/// For a flat window with two independent edges, use `filter` with both cutoffs connected.
#[kahuna_module(id = "band-pass")]
pub struct BandPass {
    filter: StateVariableFilter,

    #[input]
    input: Audio,

    #[input]
    center: Parameter<f32>,

    #[input(default = 1.0)]
    bandwidth: Parameter<f32>,

    #[output]
    out: Audio,
}

impl BandPass {
    pub fn init(module_env: &ModuleEnv) -> Self {
        Self {
            filter: StateVariableFilter::new(module_env.sample_rate),
        }
    }

    pub fn process<'a>(&mut self, inputs: &BandPassInputs<'a>, outputs: &mut BandPassOutputs<'a>) {
        outputs.out.copy_from_slice(inputs.input);
        self.filter.process_band_pass_in_place(inputs.center, inputs.bandwidth, outputs.out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kahuna_core::{KahunaModule, PortDirection, StreamKind};
    use std::f32::consts::PI;

    const SAMPLE_RATE: u32 = 48000;
    const FRAMES: usize = 4096;

    fn run(input: &[f32], center: f32, bandwidth: Option<f32>) -> Vec<f32> {
        let env = ModuleEnv { sample_rate: SAMPLE_RATE, max_buffer_size: FRAMES };
        let mut band_pass = BandPass::new(&env);

        let mut out = vec![0.0; input.len()];
        let inputs = BandPassInputs::new(input, center, bandwidth);
        let mut outputs = BandPassOutputs::new(&mut out);
        band_pass.process(&inputs, &mut outputs);

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
        assert_eq!(BandPass::ID, "band-pass");
    }

    #[test]
    fn test_ports_reflection() {
        let names: Vec<_> = BandPass::PORTS.iter().map(|port| port.name).collect();
        assert_eq!(names, ["input", "center", "bandwidth", "out"]);

        assert_eq!(BandPass::PORTS[0].stream_kind, StreamKind::Signal);
        assert_eq!(BandPass::PORTS[0].required, Some(true));

        let center = &BandPass::PORTS[1];
        assert_eq!(center.stream_kind, StreamKind::Parameter);
        assert_eq!(center.required, Some(true));
        assert!(center.value_type.matches(&kahuna_core::ValueType::of::<f32>()));

        let bandwidth = &BandPass::PORTS[2];
        assert_eq!(bandwidth.required, Some(false));
        assert!(bandwidth.value_type.matches(&kahuna_core::ValueType::of::<f32>()));

        assert_eq!(BandPass::PORTS[3].direction, PortDirection::Output);
    }

    #[test]
    fn test_passes_the_center_and_blocks_far_away_tones_and_dc() {
        let at_center = run(&sine(1000.0), 1000.0, None);
        let far_above = run(&sine(10_000.0), 1000.0, None);
        let far_below = run(&sine(50.0), 1000.0, None);
        let dc = run(&vec![1.0; FRAMES], 1000.0, None);

        assert!((settled_peak(&at_center) - 1.0).abs() < 0.05);
        assert!(settled_peak(&far_above) < 0.15);
        assert!(settled_peak(&far_below) < 0.1);
        assert!(dc[FRAMES - 1].abs() < 1e-3);
    }

    #[test]
    fn test_bandwidth_defaults_to_one_octave() {
        let tone_at_the_upper_edge = sine(1000.0 * 2.0_f32.sqrt());

        let by_default = run(&tone_at_the_upper_edge, 1000.0, None);
        let explicit = run(&tone_at_the_upper_edge, 1000.0, Some(1.0));

        assert_eq!(by_default, explicit);
    }

    #[test]
    fn test_a_narrower_band_lets_less_through_next_to_the_center() {
        let next_to_center = sine(1500.0);

        let wide = run(&next_to_center, 1000.0, Some(2.0));
        let narrow = run(&next_to_center, 1000.0, Some(0.25));

        assert!(settled_peak(&narrow) < settled_peak(&wide));
    }
}
