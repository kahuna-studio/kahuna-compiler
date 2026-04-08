pub fn add_indentation_tags(input: &str) -> String {

  let mut result = String::new();
  let mut indent_stack = vec![0];

  for (line_number, line) in input.lines().enumerate() {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
      continue;
    }

    let indent = line.chars().take_while(|c| *c == ' ').count();

    let current_indent = *indent_stack.last().unwrap();
    match indent.cmp(&current_indent) {
      std::cmp::Ordering::Greater => {

        indent_stack.push(indent);
        result.push_str("{{INDENT}}");
      }
      std::cmp::Ordering::Less => {

        let mut i = 0;
        while indent < *indent_stack.last().unwrap() {
          indent_stack.pop();
          if i > 0 {
            result.push_str("{{NEWLINE}}\n");
          }
          result.push_str("{{DEDENT}}");
          i += 1;
        }
        if indent != *indent_stack.last().unwrap() {
          panic!("Invalid indentation at line: {}", line_number);
        }
      }
      std::cmp::Ordering::Equal => {}
    }

    result.push_str(trimmed.trim_start());
    result.push_str("{{NEWLINE}}\n");
  }

  while indent_stack.len() > 1 {
    indent_stack.pop();
    result.push_str("{{DEDENT}}{{NEWLINE}}\n");
  }

  result
}
