use kahuna_compiler::compiler::ast_construction::ast::basics::File;
use kahuna_compiler::compiler::ast_construction::ast_mapper::map_to_ast;
use kahuna_compiler::compiler::source_file_parser::source_file_parser::Rule;
use kahuna_compiler::CompilationStepFailure;
use pest::iterators::Pairs;

pub fn verify_ast_construction(input: &mut Pairs<Rule>) -> Result<File, CompilationStepFailure> {

  let result = map_to_ast(input);

  if result.is_err() {
    return Err(CompilationStepFailure { error: format!("Parse error: {}", result.err().unwrap()), diff: None });
  }
  Ok(result.unwrap())
}
