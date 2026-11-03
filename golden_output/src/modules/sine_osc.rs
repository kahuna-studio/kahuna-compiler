use kahuna_core::ModuleEnv;
use kahuna_module::kahuna_module;
use kahuna_std::phase_generators::AudioRatePhase;
use std::f32::consts::PI;

#[kahuna_module]
pub struct SineOsc {
    phase: AudioRatePhase,

    #[input]
    frequency: Control,

    #[output(rate = Rate::AudioRate)]
    out: Audio,
}

impl SineOsc {
    pub fn init(module_env: &ModuleEnv) -> Self {
        Self {
            phase: AudioRatePhase::new(module_env.sample_rate)
        }
    }

    pub fn process<'a>(&mut self, inputs: &SineOscInputs<'a>, outputs: &mut SineOscOutputs<'a>) {

        self.phase.process(inputs.frequency, outputs.out);

        for x in outputs.out.iter_mut() {
            *x = (*x * 2.0 * PI).sin();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kahuna_core::{KahunaModule, PortDirection, Rate, StreamKind, ValueType};

    #[test]
    fn test_ports_reflection() {
        assert_eq!(SineOsc::PORTS.len(), 2);

        let freq_port = &SineOsc::PORTS[0];
        assert_eq!(freq_port.direction, PortDirection::Input);
        assert_eq!(freq_port.name, "frequency");
        assert_eq!(freq_port.required, Some(true));
        assert_eq!(freq_port.stream_kind, StreamKind::Parameter);
        assert_eq!(freq_port.rate, Some(Rate::ControlRate));
        assert!(freq_port.value_type.matches(&ValueType::of::<f32>()));

        let out_port = &SineOsc::PORTS[1];
        assert_eq!(out_port.direction, PortDirection::Output);
        assert_eq!(out_port.name, "out");
        assert_eq!(out_port.required, None);
        assert_eq!(out_port.stream_kind, StreamKind::Signal);
        assert_eq!(out_port.rate, Some(Rate::AudioRate));
        assert!(out_port.value_type.matches(&ValueType::of::<f32>()));
    }

    #[test]
    fn test_dsp_process() {
        let env = ModuleEnv {
            sample_rate: 48000,
            max_buffer_size: 256,
        };
        let mut osc = SineOsc::new(&env);

        let mut buffer = [0.0f32; 64];
        let inputs = SineOscInputs::new(440.0);
        let mut outputs = SineOscOutputs::new(&mut buffer);

        osc.process(&inputs, &mut outputs);

        // First sample starts at sin(0) = 0
        assert!((buffer[0] - 0.0).abs() < 1e-5);
    }
}
