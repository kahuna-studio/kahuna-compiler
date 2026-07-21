use crate::compilation_step_test_lib::model::{CompilationStepFailure, Diff};
use kahuna_compiler::testing::{AstPrinter, AstVisitor, File};

pub fn verify_ast_mapping_by_kahuna_code(input: &mut File, expected_result: Option<&str>) -> Result<(), CompilationStepFailure> {

  let mut printer = AstPrinter::new();
  printer.visit_file(&input);
  let code_generated_from_ast = printer.get_output().to_string();

  if let Some(expected) = expected_result {

    let expected = expected;

    let actual = code_generated_from_ast;

    if expected.trim() != actual.trim() {
      return Err(CompilationStepFailure {
        error: "AST to Kahuna code mapping incorrect".to_string(),
        diff: Some(Diff { expected: expected.to_string(), actual: actual.to_string() })
      });
    }
  }
  Ok(())
}
