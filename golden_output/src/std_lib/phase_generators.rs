// ============================================================
// KAHUNA STDLIB: AudioRatePhase
//
// Not written by the user — instantiated by the compiler
// whenever Audio_Rate_Phase(...) appears in Kahuna source.
//
// Owns the phase accumulator (the only state).
// Fills a &mut [f32] buffer with phase values in [0.0, 1.0).
// Expects a control-rate frequency: one f32 value per buffer.
//
// The owning module holds this as a compiler-inserted field,
// named __phase_gen_N so multiple instances per module are
// possible without collision.
// ============================================================

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

    /// Fills `out` with rising phase values advanced by `frequency / sample_rate` per sample.
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
