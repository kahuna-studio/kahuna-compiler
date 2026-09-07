use kahuna_core::{ModuleEnv, Unipolar};
use kahuna_module::kahuna_module;

#[kahuna_module]
pub struct GateEnvelope {

    #[input]
    gate: Parameter<bool>,

    #[output]
    envelope: Signal<Unipolar>
}

impl GateEnvelope {

    pub fn init(_env: &ModuleEnv) -> Self {
        Self {
        }
    }

    pub fn process<'a>(&mut self, inputs: &Inputs<'a>, outputs: &mut Outputs<'a>) {
        outputs.envelope.fill(if inputs.gate { 1.0 } else { 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kahuna_core::{KahunaModule, PortDirection, Rate, StreamKind, ValueType};

    #[test]
    fn test_gate_envelope_ports() {
        assert_eq!(GateEnvelope::PORTS.len(), 2);

        let gate_port = &GateEnvelope::PORTS[0];
        assert_eq!(gate_port.direction, PortDirection::Input);
        assert_eq!(gate_port.name, "gate");
        assert_eq!(gate_port.required, Some(true));
        assert_eq!(gate_port.stream_kind, StreamKind::Parameter);
        assert_eq!(gate_port.rate, Some(Rate::ControlRate));
        assert!(gate_port.value_type.matches(&ValueType::of::<bool>()));

        let env_port = &GateEnvelope::PORTS[1];
        assert_eq!(env_port.direction, PortDirection::Output);
        assert_eq!(env_port.name, "envelope");
        assert_eq!(env_port.required, None);
        assert_eq!(env_port.stream_kind, StreamKind::Signal);
        assert_eq!(env_port.rate, Some(Rate::AudioRate));
        assert!(env_port.value_type.matches(&ValueType::of::<f32>()));
    }

    #[test]
    fn test_gate_envelope_process() {
        let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
        let mut gate = GateEnvelope::new(&env);

        let mut buffer = [0.0f32; 16];
        let inputs = Inputs::new(false);
        let mut outputs = Outputs::new(&mut buffer);

        gate.process(&inputs, &mut outputs);
        assert!(buffer.iter().all(|&s| s == 0.0));
    }
}
