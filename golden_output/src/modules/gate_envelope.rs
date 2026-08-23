use kahuna_module::{kahuna_module, ModuleEnv, Range, Rate};

kahuna_module! {
    struct GateEnvelope {}

    inputs {
        @gate: f32 [rate = Rate::ControlRate, range = Range::Float(0.0, 1.0)],
    }

    outputs {
        ~envelope: f32 [rate = Rate::AudioRate, range = Range::Float(0.0, 1.0)],
    }
}

impl GateEnvelope {
    pub fn init(&mut self, _env: &ModuleEnv) {}

    pub fn process<'a>(&mut self, inputs: &Inputs<'a>, outputs: &mut Outputs<'a>) {
        outputs.envelope.fill(inputs.gate);
    }
}

#[cfg(test)]
mod tests {
    use kahuna_module::{PortDir, PortKind, KahunaModule};
    use super::*;

    #[test]
    fn test_gate_envelope_ports() {
        assert_eq!(GateEnvelope::PORTS.len(), 2);

        let gate_port = &GateEnvelope::PORTS[0];
        assert_eq!(gate_port.name, "gate");
        assert_eq!(gate_port.dir, PortDir::Input);
        assert_eq!(gate_port.kind, PortKind::Parameter);
        assert_eq!(gate_port.rate, Some(Rate::ControlRate));
        assert_eq!(gate_port.range, Some(Range::Float(0.0, 1.0)));

        let env_port = &GateEnvelope::PORTS[1];
        assert_eq!(env_port.name, "envelope");
        assert_eq!(env_port.dir, PortDir::Output);
        assert_eq!(env_port.kind, PortKind::Signal);
        assert_eq!(env_port.rate, Some(Rate::AudioRate));
    }

    #[test]
    fn test_gate_envelope_process() {
        let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
        let mut gate = GateEnvelope::new(&env);

        let mut buffer = [0.0f32; 16];
        let inputs = Inputs { gate: 1.0, _phantom: std::marker::PhantomData };
        let mut outputs = Outputs { envelope: &mut buffer, _phantom: std::marker::PhantomData };

        gate.process(&inputs, &mut outputs);
        assert!(buffer.iter().all(|&s| s == 1.0));
    }
}
