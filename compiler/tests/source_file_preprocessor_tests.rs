use kahuna_compiler::internal_pre_process;
use kahuna_compiler::CompilationStepFailure;
use kahuna_compiler::Diff;

pub fn verify_source_file_pre_processing(input: &str, expected_result: Option<&str>) -> Result<String, CompilationStepFailure> {

  let result = internal_pre_process(input);

  if let Some(expected) = expected_result {

    let expected = expected;
    let actual = result.clone();

    if expected.trim() != actual.trim() {
      return Err(CompilationStepFailure {
        error: "Preprocessing failed".to_string(),
        diff: Some(Diff { expected: expected.to_string(), actual: actual.to_string() })
      });
    }
  }
  Ok(result)
}
