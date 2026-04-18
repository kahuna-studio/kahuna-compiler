use kahuna_compiler::compiler::source_file_parser::source_file_parser::Rule;
use kahuna_compiler::internal_parse_source_file;
use pest::iterators::{Pair, Pairs};
use kahuna_compiler::CompilationStepFailure;
use kahuna_compiler::Diff;

pub fn verify_source_file_parsing<'a>(input: &'a str, expected_result: Option<&str>) -> Result<Pairs<'a, Rule>, CompilationStepFailure> {

  let result = internal_parse_source_file(input);

  if result.is_err() {
    return Err(CompilationStepFailure { error: format!("Parse error: {}", result.err().unwrap()), diff: None });
  }

  if let Some(expected) = expected_result {

    let expected = expected;
    let actual = format(&result);

    if expected.trim() != actual.trim() {
      return Err(CompilationStepFailure {
        error: "Parse tree mismatch".to_string(),
        diff: Some(Diff { expected: expected.to_string(), actual: actual.to_string() })
      });
    }
  }
  Ok(result.unwrap())
}

fn format(result: &Result<Pairs<Rule>, pest::error::Error<Rule>>) -> String {
  let file_pair = result.clone().unwrap().next().unwrap();
  format_pair(file_pair.clone(), 0)
}

fn format_pair(pair: Pair<Rule>, indent: usize) -> String {

  let mut output = String::new();

  let padding = "  ".repeat(indent);
  let mut inner = pair.clone().into_inner();

  if inner.next().is_some() {
    // Has children: print only the rule name
    output.push_str(&format!("{}{:?}\n", padding, pair.as_rule()));

    for child in pair.into_inner() {
      let child_result = format_pair(child, indent + 1);
      output.push_str(&child_result);
    }
  } else if pair.as_rule() != Rule::EOI {
    // Leaf node: print rule name and text
    output.push_str(&format!(
      "{}{:?}: {}\n",
      padding,
      pair.as_rule(),
      pair.as_str()
    ));
  }
  output
}
