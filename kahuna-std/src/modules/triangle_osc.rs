use crate::phase_generators::AudioRatePhase;
use kahuna_core::ModuleEnv;
use kahuna_unit::kahuna_module;

/// Triangle wave. Starts at 0, peaks at a quarter period, bottoms out at three quarters.
/// Its harmonics fall off with 1/n², so it needs no band-limiting.
#[kahuna_module(id = "triangle")]
pub struct TriangleOsc {
  phase: AudioRatePhase,

  #[input]
  frequency: Control,

  #[output(rate = Rate::AudioRate)]
  out: Audio,
}

impl TriangleOsc {
  pub fn init(module_env: &ModuleEnv) -> Self {
    Self {
      phase: AudioRatePhase::new(module_env.sample_rate),
    }
  }

  pub fn process<'a>(&mut self, inputs: &TriangleOscInputs<'a>, outputs: &mut TriangleOscOutputs<'a>) {
    self.phase.process(inputs.frequency, outputs.out);

    for sample in outputs.out.iter_mut() {
      *sample = triangle_sample(*sample);
    }
  }
}

fn triangle_sample(phase: f32) -> f32 {
  if phase < 0.25 {
    4.0 * phase
  } else if phase < 0.75 {
    2.0 - 4.0 * phase
  } else {
    4.0 * phase - 4.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use kahuna_core::{KahunaModule, Rate};

  #[test]
  fn test_id() {
    assert_eq!(TriangleOsc::ID, "triangle");
  }

  #[test]
  fn test_ports_reflection() {
    assert_eq!(TriangleOsc::PORTS.len(), 2);
    assert_eq!(TriangleOsc::PORTS[0].name, "frequency");
    assert_eq!(TriangleOsc::PORTS[1].name, "out");
    assert_eq!(TriangleOsc::PORTS[1].rate, Some(Rate::AudioRate));
  }

  #[test]
  fn test_shape_over_one_period() {
    let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
    let mut osc = TriangleOsc::new(&env);

    // 480 Hz at 48 kHz: one period is exactly 100 samples.
    let mut buffer = [0.0f32; 100];
    let inputs = TriangleOscInputs::new(480.0);
    let mut outputs = TriangleOscOutputs::new(&mut buffer);

    osc.process(&inputs, &mut outputs);

    assert!(buffer[0].abs() < 1e-5);
    assert!((buffer[25] - 1.0).abs() < 1e-4);
    assert!(buffer[50].abs() < 1e-4);
    assert!((buffer[75] + 1.0).abs() < 1e-4);
  }
}
