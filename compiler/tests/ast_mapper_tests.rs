use kahuna_compiler::compiler::source_file_parser::source_file_parser::Rule;
use pest::iterators::Pairs;
use kahuna_compiler::compiler::ast_construction::ast::basics::File;
use kahuna_compiler::compiler::ast_construction::ast_mapper::map_to_ast;
use kahuna_compiler::compiler::ast_construction::ast::ast_printer::AstVisitor;
use kahuna_compiler::{CompilationStepFailure, Diff};

pub fn verify_ast_mapping(input: &mut Pairs<Rule>, expected_result: Option<&str>) -> Result<File, CompilationStepFailure> {

  let result = map_to_ast(input);


  if result.is_err() {
    return Err(CompilationStepFailure { error: format!("Parse error: {}", result.err().unwrap()), diff: None });
  }

  let file = result.unwrap();

  let mut printer = kahuna_compiler::compiler::ast_construction::ast::ast_printer::AstPrinter::new();
  printer.visit_file(&file);
  let code_generated_from_ast = printer.get_output().to_string();

  if let Some(expected) = expected_result {

    let expected = expected;

    let actual = code_generated_from_ast;

    if expected.trim() != actual.trim() {
      return Err(CompilationStepFailure {
        error: "AST mapping incorrect".to_string(),
        diff: Some(Diff { expected: expected.to_string(), actual: actual.to_string() })
      });
    }
  }
  Ok(file)
}
