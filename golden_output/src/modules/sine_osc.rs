use std::f32::consts::PI;
use crate::std_lib::phase_generators::AudioRatePhase;

pub struct SineOsc {

    phase: AudioRatePhase,
    frequency: f32,
}

impl SineOsc {
    pub fn new(sample_rate: u32, _max_buffer_size: usize) -> Self {
        Self {
            phase: AudioRatePhase::new(sample_rate),
            frequency: 440.0,
        }
    }

    pub fn process(
        &mut self,
        frequency: Option<f32>,
        out: &mut [f32],
    ) {
        if let Some(freq) = frequency {
            self.frequency = freq;
        }

        self.phase.process(self.frequency, out);

        for x in out.iter_mut() {
            *x = (*x * 2.0 * PI).sin();
        }
    }
}
