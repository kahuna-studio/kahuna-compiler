pub struct Multiply {
    input_count: usize,
}

impl Multiply {
    pub fn new(_sample_rate: u32, _max_buffer_size: usize, input_count: usize) -> Self {
        Self { input_count }
    }

    pub fn process(
        &mut self,
        input_0: &[f32],
        input_1: &[f32],
        output: &mut [f32],
    ) {
        for i in 0..output.len() {
            output[i] = input_0[i] * input_1[i];
        }
    }

    pub fn input_index(&self, name: &str) -> Option<usize> {
        name.strip_prefix("in")?.parse().ok()
    }

    pub fn output_index(&self, name: &str) -> Option<usize> {
        match name {
            "out" => Some(0),
            _ => None,
        }
    }
}
