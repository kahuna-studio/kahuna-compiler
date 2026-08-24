pub struct Multiply {
}

impl Multiply {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self {}
    }

    pub fn process(
        &self,
        inputs: &[&[f32]],
        output: &mut [f32],
    ) {
        for sample_idx in 0..output.len() {
            output[sample_idx] = inputs.iter().map(|ch| ch[sample_idx]).product();
        }
    }
}
