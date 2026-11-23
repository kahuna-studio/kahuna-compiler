use crate::phase_generators::AudioRatePhase;
use crate::poly_blep::poly_blep;
use kahuna_core::ModuleEnv;
use kahuna_unit::kahuna_module;

/// Pulse limits: a pulse width of exactly 0 or 1 would be silence (a constant).
const MIN_PULSE_WIDTH: f32 = 0.01;
const MAX_PULSE_WIDTH: f32 = 0.99;

/// Band-limited pulse wave. `pulse_width` is the share of the period spent high:
/// 0.5 is a square, smaller values sound thinner and more nasal.
#[kahuna_module(id = "square")]
pub struct SquareOsc {
  sample_rate: f32,
  phase: AudioRatePhase,

  #[input]
  frequency: Control,

  #[input(default = 0.5)]
  pulse_width: Control,

  #[output(rate = Rate::AudioRate)]
  out: Audio,
}

impl SquareOsc {
  pub fn init(module_env: &ModuleEnv) -> Self {
    Self {
      sample_rate: module_env.sample_rate as f32,
      phase: AudioRatePhase::new(module_env.sample_rate),
    }
  }

  pub fn process<'a>(&mut self, inputs: &SquareOscInputs<'a>, outputs: &mut SquareOscOutputs<'a>) {
    let increment = inputs.frequency / self.sample_rate;
    let pulse_width = inputs.pulse_width.clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH);

    self.phase.process(inputs.frequency, outputs.out);

    for sample in outputs.out.iter_mut() {
      *sample = square_sample(*sample, increment, pulse_width);
    }
  }
}

/// Naive pulse with a PolyBLEP correction at both edges: the rising one at phase 0,
/// the falling one at `pulse_width`.
fn square_sample(phase: f32, increment: f32, pulse_width: f32) -> f32 {
  let naive = if phase < pulse_width { 1.0 } else { -1.0 };
  let falling_edge_phase = (phase - pulse_width).rem_euclid(1.0);

  naive + poly_blep(phase, increment) - poly_blep(falling_edge_phase, increment)
}

#[cfg(test)]
mod tests {
  use super::*;
  use kahuna_core::{KahunaModule, PortDirection, StreamKind};

  #[test]
  fn test_id() {
    assert_eq!(SquareOsc::ID, "square");
  }

  #[test]
  fn test_ports_reflection() {
    assert_eq!(SquareOsc::PORTS.len(), 3);

    let frequency = &SquareOsc::PORTS[0];
    assert_eq!(frequency.name, "frequency");
    assert_eq!(frequency.required, Some(true));

    let pulse_width = &SquareOsc::PORTS[1];
    assert_eq!(pulse_width.name, "pulse_width");
    assert_eq!(pulse_width.direction, PortDirection::Input);
    assert_eq!(pulse_width.stream_kind, StreamKind::Parameter);
    assert_eq!(pulse_width.required, Some(false));

    assert_eq!(SquareOsc::PORTS[2].name, "out");
  }

  #[test]
  fn test_pulse_width_defaults_to_half() {
    let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
    let mut osc = SquareOsc::new(&env);

    // 480 Hz at 48 kHz: one period is exactly 100 samples.
    let mut buffer = [0.0f32; 100];
    let inputs = SquareOscInputs::new(480.0, None);
    let mut outputs = SquareOscOutputs::new(&mut buffer);

    osc.process(&inputs, &mut outputs);

    let high = buffer.iter().filter(|x| **x > 0.0).count();
    assert!((high as i32 - 50).abs() <= 2, "high samples: {high}");
  }

  #[test]
  fn test_narrow_pulse_is_high_for_less_of_the_period() {
    let env = ModuleEnv { sample_rate: 48000, max_buffer_size: 256 };
    let mut osc = SquareOsc::new(&env);

    let mut buffer = [0.0f32; 100];
    let inputs = SquareOscInputs::new(480.0, Some(0.1));
    let mut outputs = SquareOscOutputs::new(&mut buffer);

    osc.process(&inputs, &mut outputs);

    let high = buffer.iter().filter(|x| **x > 0.0).count();
    assert!((high as i32 - 10).abs() <= 2, "high samples: {high}");
  }
}
