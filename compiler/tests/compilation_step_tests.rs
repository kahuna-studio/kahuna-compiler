mod compilation_step_test_lib;

use std::path::PathBuf;
use indexmap::IndexMap;
use similar::{ChangeTag, TextDiff};
use crate::compilation_step_test_lib::model::{CompilationStep, CompilationStepResult};
use crate::compilation_step_test_lib::test_runner::{load_cases, run_test_case};

#[test]
fn compilation_step_tests() {
  let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("compilation_step_test_cases");
  let cases = load_cases(dir.to_str().unwrap());

  assert!(
    !cases.is_empty(),
    "No .test files found in {}",
    dir.display()
  );

  let mut succeeded = true;
  let mut results: IndexMap<String, IndexMap<CompilationStep, CompilationStepResult>> =
    IndexMap::new();
  for case in cases {
    let test_case_result = run_test_case(&case);
    results.insert(case.name, test_case_result);
  }

  results.iter().for_each(|(name, result)| {
    let output = format!("Test case '{}'", name);
    result
      .iter()
      .for_each(|(&step, step_result)| match step_result {
        CompilationStepResult::Success(true) => {
          println!("\x1b[32m✅ {} - {:?}\x1b[0m", output, step);
        }
        CompilationStepResult::Success(false) => {
          println!(
            "\x1b[33m✅ {} - {:?} (no specification defined)\x1b[0m",
            output, step
          );
        }
        CompilationStepResult::Failure(failure) => {
          succeeded = false;
          println!("\x1b[31m❌ {} - {:?}\x1b[0m", output, step);
          println!("   Error: {}", failure.error);

          if failure.diff.is_some() {
            let diff = failure.diff.as_ref().unwrap();
            println!("   Diff:\n");
            let diff = TextDiff::from_lines(diff.expected.clone(), diff.actual.clone());

            for change in diff.iter_all_changes() {
              let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
              };
              print!("{}{}", sign, change);
            }
            println!("")
          }
          println!("");
        }
      });
  });

  if !succeeded {
    std::panic::resume_unwind(Box::new(()));
  }
}
