use kahuna_compiler::compiler::ast_construction::ast::basics::File;
use kahuna_compiler::compiler::ast_to_json_serializer::ast_to_json_serializer::AstJsonSerializer;
use kahuna_compiler::{CompilationStepFailure, Diff};

pub fn verify_ast_mapping_by_json(input: &mut File, expected_result: Option<&str>) -> Result<(), CompilationStepFailure> {

  let json_string = AstJsonSerializer::serialize_file_to_string(input).unwrap();

  println!("{:?}", json_string.clone());

  if let Some(expected) = expected_result {

    let actual = json_string;

    if expected.trim() != actual.trim() {
      return Err(CompilationStepFailure {
        error: "AST to JSON mapping incorrect".to_string(),
        diff: Some(Diff { expected: expected.to_string(), actual: actual.to_string() })
      });
    }
  }
  Ok(())
}
