#[derive(Default)]
pub struct AudioRatePhase {
    sample_rate: u32,
    phase: f32,
}

impl AudioRatePhase {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            phase: 0.0
        }
    }

    pub fn process(&mut self, frequency: f32, out: &mut [f32]) {
        let increment = frequency / self.sample_rate as f32;
        for sample in out.iter_mut() {
            *sample = self.phase;
            self.phase += increment;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}
