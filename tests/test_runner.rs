mod parser_tests;
mod preprocessor_tests;

use std::collections::HashMap;
use parser_tests::verify_parsing;
use similar::{ChangeTag, TextDiff};
use indexmap::IndexMap;
use std::fs;
use std::path::Path;

struct TestCase {
    name: String,
    source: String,
    sections: HashMap<CompilationStep, String>,
}
use std::str::FromStr;
use crate::preprocessor_tests::verify_preprocessing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CompilationStep {
    PreProcessing,
    Parse,
    ConstructAst,
}

impl FromStr for CompilationStep {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "pre-process" => Ok(CompilationStep::PreProcessing),
            "parse" => Ok(CompilationStep::Parse),
            "constructAst" => Ok(CompilationStep::ConstructAst),
            other => Err(format!("Unknown compilation step: '{other}'")),
        }
    }
}

enum CompilationStepResult {
    Success(bool),
    Failure(CompilationStepFailure),
}

#[derive(Clone, Debug)]
struct CompilationStepFailure {
    error: String,
    diff: Option<Diff>,
}

#[derive(Clone, Debug)]
struct Diff {
  expected: String,
   actual: String,
}

#[test]
fn all_cases() {
    let dir = Path::new("tests/cases");
    let cases = load_cases(dir.to_str().unwrap());

    assert!(
        !cases.is_empty(),
        "No .test files found in {}",
        dir.display()
    );

    let mut succeeded = true;
    let mut results: IndexMap<String, IndexMap<CompilationStep, CompilationStepResult>> = IndexMap::new();
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
                println!("\x1b[33m✅ {} - {:?} (no specification defined)\x1b[0m", output, step);
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

fn run_test_case(test_case: &TestCase) -> IndexMap<CompilationStep, CompilationStepResult> {

    let mut results: IndexMap<CompilationStep, CompilationStepResult> = IndexMap::new();

    // ----------------------- Pre-processing -----------------------

    let step = CompilationStep::PreProcessing;
    let source = test_case.source.clone();
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_preprocessing(&source, expected);

    update_results(&mut results, result.clone().map(|_| ()), step, expected.is_some());
    if result.is_err() {
      return results
    }

    // ----------------------- Parsing -----------------------

    let step = CompilationStep::Parse;
    let source = result.unwrap();
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_parsing(source.as_str(), expected);

    update_results(&mut results, result.clone().map(|_| ()), step, expected.is_some());
    if result.is_err() {
      return results
    }

    results
}

fn load_cases(dir: &str) -> Vec<TestCase> {
    let mut cases = Vec::new();

    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("test") {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap();
        cases.push(parse_case(
            path.file_stem().unwrap().to_string_lossy().to_string(),
            &content,
        ));
    }

    cases
}

fn parse_case(name: String, content: &str) -> TestCase {
    let mut parts = content.split("\n===");
    let source = parts.next().unwrap().trim().to_string();

    let mut sections: HashMap<CompilationStep, String> = HashMap::new();

    for part in parts {
        let mut lines = part.trim().lines();
        let key = lines.next().unwrap().trim().to_string();
        let compilation_step = key
            .parse::<CompilationStep>()
            .unwrap_or_else(|_| panic!("Invalid compilation step '{key}' in test case '{name}'"));
        let value = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        sections.insert(compilation_step, value);
    }

    TestCase {
        name,
        source,
        sections,
    }
}

fn update_results(results: &mut IndexMap<CompilationStep, CompilationStepResult>, result: Result<(), CompilationStepFailure>, compilation_step: CompilationStep, has_spec: bool) {
  results.insert(
    compilation_step,
    match result {
      Ok(_) => CompilationStepResult::Success(has_spec),
      Err(failure) => CompilationStepResult::Failure(failure),
    },
  );
}
