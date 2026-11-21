mod patch;

use crate::patch::Patch;
use kahuna_runtime::run_standalone;
use std::process::ExitCode;

fn main() -> ExitCode {
  let result = run_standalone(2048, |sample_rate, max_buffer_size, factory| {
    Patch::new(sample_rate, max_buffer_size, factory)
  });
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("Error: {error}");
      ExitCode::FAILURE
    }
  }
}
