use kahuna_core::ModuleEnv;

pub struct Multiply {
}

impl Multiply {
    pub fn new(_module_env: &ModuleEnv) -> Self {
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
