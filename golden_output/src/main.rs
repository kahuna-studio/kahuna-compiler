mod modules;
mod patch;

use kahuna_runtime::run_standalone;
use crate::patch::Patch;

fn main() {
  run_standalone(2048, |sample_rate, max_buffer_size, factory| {
    Patch::new(sample_rate, max_buffer_size, factory)
  });
}
