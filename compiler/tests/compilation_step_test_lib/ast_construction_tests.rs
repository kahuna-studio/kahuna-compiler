use crate::compilation_step_test_lib::model::CompilationStepFailure;
use kahuna_compiler::compiler::testing::{map_to_ast, File, Rule};
use pest::iterators::Pairs;

pub fn verify_ast_construction(input: &mut Pairs<Rule>) -> Result<File, CompilationStepFailure> {

  let result = map_to_ast(input);

  if result.is_err() {
    return Err(CompilationStepFailure { error: format!("Parse error: {}", result.err().unwrap()), diff: None });
  }
  Ok(result.unwrap())
}
