use kahuna_core::{ModuleEnv, Unipolar};
use kahuna_module::kahuna_module;

#[derive(Clone, Copy, PartialEq)]
enum Stage {
  Idle,
  Attack,
  Decay,
  Sustain,
  Release,
}

#[kahuna_module]
pub struct AdsrEnvelope {
  sample_rate: f32,
  stage: Stage,
  level: f32,

  #[input]
  gate: Parameter<bool>,

  #[input(default = 0.01)]
  attack: Parameter<f32>,

  #[input(default = 0.1)]
  decay: Parameter<f32>,

  #[input(default = 0.7)]
  sustain: Parameter<f32>,

  #[input(default = 0.2)]
  release: Parameter<f32>,

  #[output]
  envelope: Signal<Unipolar>,
}

impl AdsrEnvelope {
  pub fn init(env: &ModuleEnv) -> Self {
    Self {
      sample_rate: env.sample_rate as f32,
      stage: Stage::Idle,
      level: 0.0,
    }
  }

  pub fn process<'a>(&mut self, inputs: &Inputs<'a>, outputs: &mut Outputs<'a>) {

    let attack_step = 1.0 / (inputs.attack * self.sample_rate).max(1.0);
    let decay_step = 1.0 / (inputs.decay * self.sample_rate).max(1.0);
    let release_step = 1.0 / (inputs.release * self.sample_rate).max(1.0);

    if inputs.gate {
      if self.stage == Stage::Idle || self.stage == Stage::Release {
        self.stage = Stage::Attack;
      }
    } else if self.stage != Stage::Idle {
      self.stage = Stage::Release;
    }

    for sample in outputs.envelope.iter_mut() {
      
      match self.stage {
        Stage::Idle => self.level = 0.0,
        Stage::Attack => {
          self.level += attack_step;
          if self.level >= 1.0 {
            self.level = 1.0;
            self.stage = Stage::Decay;
          }
        }
        Stage::Decay => {
          self.level -= decay_step * (1.0 - inputs.sustain);
          if self.level <= inputs.sustain {
            self.level = inputs.sustain;
            self.stage = Stage::Sustain;
          }
        }
        Stage::Sustain => self.level = inputs.sustain,
        Stage::Release => {
          self.level -= release_step;
          if self.level <= 0.0 {
            self.level = 0.0;
            self.stage = Stage::Idle;
          }
        }
      }

      *sample = self.level;
    }
  }
}
