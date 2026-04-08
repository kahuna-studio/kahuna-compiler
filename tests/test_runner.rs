mod parser_tests;
use parser_tests::verify_parsing;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

struct TestCase {
    name: String,
    source: String,
    sections: HashMap<CompilationStep, String>,
}
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CompilationStep {
    Parse,
    ConstructAst,
}

impl FromStr for CompilationStep {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "parse" => Ok(CompilationStep::Parse),
            "constructAst" => Ok(CompilationStep::ConstructAst),
            other => Err(format!("Unknown compilation step: '{other}'")),
        }
    }
}

enum CompilationStepResult {
    Success,
    Failure(String, Option<(String, String)>),
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
    let mut results: HashMap<String, HashMap<CompilationStep, CompilationStepResult>> =
        HashMap::new();
    for case in cases {
        let test_case_result = run_test_case(&case);
        results.insert(case.name, test_case_result);
    }

    results.iter().for_each(|(name, result)| {
        let output = format!("Test case '{}'", name);
        result
            .iter()
            .for_each(|(&step, step_result)| match step_result {
                CompilationStepResult::Success => {
                    println!("\x1b[32m✅ {} - {:?}\x1b[0m", output, step);
                }
                CompilationStepResult::Failure(error, diff) => {
                    succeeded = false;
                    println!("\x1b[31m❌ {} - {:?}\x1b[0m", output, step);
                    println!("   Error: {}", error);

                    if let Some((expected, actual)) = diff {
                        println!("   Diff:\n");
                        let diff = TextDiff::from_lines(expected, actual);

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

fn run_test_case(test_case: &TestCase) -> HashMap<CompilationStep, CompilationStepResult> {
    let mut results: HashMap<CompilationStep, CompilationStepResult> = HashMap::new();

    let parsing_result = verify_parsing(
        &test_case.source,
        test_case
            .sections
            .get(&CompilationStep::Parse)
            .map(|s| s.as_str()),
    );
    results.insert(
        CompilationStep::Parse,
        match &parsing_result {
            Ok(_) => CompilationStepResult::Success,
            Err(err) => CompilationStepResult::Failure(err.0.clone(), err.1.clone()),
        },
    );

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
            .unwrap_or_else(|err| panic!("Invalid compilation step '{key}' in test case '{name}'"));
        let value = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        sections.insert(compilation_step, value);
    }

    TestCase {
        name,
        source,
        sections,
    }
}
