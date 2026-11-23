use crate::phase_generators::AudioRatePhase;
use crate::poly_blep::poly_blep;
use kahuna_core::ModuleEnv;
use kahuna_unit::kahuna_module;

/// Band-limited sawtooth: rises from -1 to 1 over one period, then jumps back.
#[kahuna_module(id = "saw")]
pub struct SawOsc {
  sample_rate: f32,
  phase: AudioRatePhase,

  #[input]
  frequency: Control,

  #[output(rate = Rate::AudioRate)]
  out: Audio,
}

impl SawOsc {
  pub fn init(module_env: &ModuleEnv) -> Self {
    Self {
      sample_rate: module_env.sample_rate as f32,
      phase: AudioRatePhase::new(module_env.sample_rate),
    }
  }

  pub fn process<'a>(&mut self, inputs: &SawOscInputs<'a>, outputs: &mut SawOscOutputs<'a>) {
    let increment = inputs.frequency / self.sample_rate;

    self.phase.process(inputs.frequency, outputs.out);

    for sample in outputs.out.iter_mut() {
      *sample = saw_sample(*sample, increment);
    }
  }
}

fn saw_sample(phase: f32, increment: f32) -> f32 {
  2.0 * phase - 1.0 - poly_blep(phase, increment)
}

#[cfg(test)]
mod tests {
  use super::*;
  use kahuna_core::{KahunaModule, PortDirection, Rate, StreamKind};

  #[test]
  fn test_id() {
    assert_eq!(SawOsc::ID, "saw");
  }

  #[test]
  fn test_ports_reflection() {
    assert_eq!(SawOsc::PORTS.len(), 2);
    assert_eq!(SawOsc::PORTS[0].name, "frequency");
    assert_eq!(SawOsc::PORTS[0].direction, PortDirection::Input);
    assert_eq!(SawOsc::PORTS[0].stream_kind, StreamKind::Parameter);
    assert_eq!(SawOsc::PORTS[1].name, "out");
    assert_eq!(SawOsc::PORTS[1].rate, Some(Rate::AudioRate));
  }

  #[test]
  fn test_dsp_process() {
    let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
    let mut osc = SawOsc::new(&env);

    let mut buffer = [0.0f32; 100];
    let inputs = SawOscInputs::new(480.0);
    let mut outputs = SawOscOutputs::new(&mut buffer);

    osc.process(&inputs, &mut outputs);

    // The correction cancels the jump at phase 0, so the cycle starts at 0.
    assert!(buffer[0].abs() < 1e-5);
    // Halfway through the period the saw crosses 0 again.
    assert!(buffer[50].abs() < 0.05);
    assert!(buffer.iter().all(|x| x.abs() <= 1.0 + 1e-3));
  }
}
