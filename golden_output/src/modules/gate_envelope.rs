pub struct GateEnvelope {
}

impl GateEnvelope {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self {}
    }

    pub fn process(
        &mut self,
        gate: f32,
        output: &mut [f32],
    ) {
        output.fill(gate);
    }
}
