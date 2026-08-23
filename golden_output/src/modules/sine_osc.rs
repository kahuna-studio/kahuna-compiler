use crate::std_lib::phase_generators::AudioRatePhase;
use kahuna_module::{kahuna_module, ModuleEnv, Range, Rate};
use std::f32::consts::PI;

kahuna_module! {
    struct SineOsc {
        phase: AudioRatePhase,
        frequency: f32,
    }

    inputs {
        @frequency: Option<f32> [rate = Rate::ControlRate, range = Range::Float(20.0, 20000.0)],
    }

    outputs {
        ~out: f32 [rate = Rate::AudioRate, range = Range::Float(-1.0, 1.0)],
    }
}

impl SineOsc {

    pub fn init(module_env: &ModuleEnv) -> Self {
        Self {
            phase: AudioRatePhase::new(module_env.sample_rate),
            frequency: 440.0,
        }
    }

    pub fn process<'a>(&mut self, inputs: &Inputs<'a>, outputs: &mut Outputs<'a>) {
        if let Some(freq) = inputs.frequency {
            self.frequency = freq;
        } else {

        }

        self.phase.process(self.frequency, outputs.out);

        for x in outputs.out.iter_mut() {
            *x = (*x * 2.0 * PI).sin();
        }
    }
}

#[cfg(test)]
mod tests {
    use kahuna_module::{PortDir, PortKind, PortType, KahunaModule};
    use super::*;

    #[test]
    fn test_ports_reflection() {
        assert_eq!(SineOsc::PORTS.len(), 2);

        let freq_port = &SineOsc::PORTS[0];
        assert_eq!(freq_port.direction, PortDir::Input);
        assert_eq!(freq_port.name, "frequency");
        assert_eq!(freq_port.required, Some(false));
        assert_eq!(freq_port.stream_type, PortKind::Parameter);
        assert_eq!(freq_port.rate, Some(Rate::ControlRate));
        assert_eq!(freq_port.range, Some(Range::Float(20.0, 20000.0)));
        assert!(freq_port.value_type.matches(&PortType::of::<f32>()));

        let out_port = &SineOsc::PORTS[1];
        assert_eq!(out_port.direction, PortDir::Output);
        assert_eq!(out_port.name, "out");
        assert_eq!(out_port.required, None);
        assert_eq!(out_port.stream_type, PortKind::Signal);
        assert_eq!(out_port.rate, Some(Rate::AudioRate));
        assert_eq!(out_port.range, Some(Range::Float(-1.0, 1.0)));
        assert!(out_port.value_type.matches(&PortType::of::<f32>()));
    }

    #[test]
    fn test_dsp_process() {
        let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
        let mut osc = SineOsc::new(&env);

        let mut buffer = [0.0f32; 64];
        let inputs = Inputs::new(Some(440.0));
        let mut outputs = Outputs::new(&mut buffer);

        osc.process(&inputs, &mut outputs);

        // First sample starts at sin(0) = 0
        assert!((buffer[0] - 0.0).abs() < 1e-5);
    }
}
