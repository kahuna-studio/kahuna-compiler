use pest::iterators::{Pair, Pairs};
use kahuna_compiler::compiler::lexer::lexer::Rule;
use kahuna_compiler::internal_parse;

pub fn verify_parsing<'a>(input: &'a str, expected_result: Option<&str>) -> Result<Pairs<'a, Rule>, (String, Option<(String, String)>)> {
  let result = internal_parse(input);

  if result.is_err() {
    return Err((format!("Parse error: {}", result.err().unwrap()), None));
  }

  let parse_result = result.clone().unwrap();

  let top_pairs: Vec<Pair<Rule>> = parse_result.clone().collect();
  assert_eq!(top_pairs.len(), 1);

  let file_pair = &top_pairs[0];
  assert_eq!(file_pair.as_rule(), Rule::file);

  let mut output = String::new();

  for pair in file_pair.clone().into_inner() {
    format_pair(pair, 0, &mut output);
  }

  let output = output.trim();

  if let Some(expected) = expected_result {
    let expected = expected.trim();

    if expected != output {
      return Err((
        "Parse tree mismatch".to_string(),
        Some((expected.to_string(), output.to_string()))));
    }
  }
  Ok(result.unwrap())
}

fn format_pair(pair: Pair<Rule>, indent: usize, out: &mut String) {
  let padding = "  ".repeat(indent);
  let mut inner = pair.clone().into_inner();

  if inner.next().is_some() {
    // Has children: print only the rule name
    out.push_str(&format!("{}{:?}\n", padding, pair.as_rule()));

    for child in pair.into_inner() {
      format_pair(child, indent + 1, out);
    }
  } else if pair.as_rule() != Rule::EOI {
    // Leaf node: print rule name and text
    out.push_str(&format!(
      "{}{:?}: {}\n",
      padding,
      pair.as_rule(),
      pair.as_str()
    ));
  }
}
