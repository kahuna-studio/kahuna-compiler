mod compilation_step_test_lib;

use std::path::PathBuf;
use indexmap::IndexMap;
use similar::{ChangeTag, TextDiff};
use crate::compilation_step_test_lib::model::{CompilationStep, CompilationStepResult, FileType};
use crate::compilation_step_test_lib::test_runner::{load_cases, run_patch_code_test_case, run_source_code_test_case};

#[test]
fn compilation_step_tests() {
  let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("compilation_step_test_cases");
  let source_code_dir = dir.join("source_code");
  let patch_code_dir = dir.join("patch_code");
  
  let source_code_cases = load_cases(FileType::SourceCode, source_code_dir.to_str().unwrap());
  let patch_code_cases = load_cases(FileType::PatchCode, patch_code_dir.to_str().unwrap());

  assert!(
    !source_code_cases.is_empty(),
    "No *.test files found in {}",
    source_code_dir.display()
  );
  assert!(
    !patch_code_cases.is_empty(),
    "No *.test files found in {}",
    patch_code_dir.display()
  );

  let mut succeeded = true;
  let mut results: IndexMap<String, IndexMap<CompilationStep, CompilationStepResult>> =
    IndexMap::new();
  for case in source_code_cases {
    let test_case_result = run_source_code_test_case(&case);
    results.insert(case.name, test_case_result);
  }
  for case in patch_code_cases {
    let test_case_result = run_patch_code_test_case(&case);
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
