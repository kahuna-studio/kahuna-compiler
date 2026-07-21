use crate::compilation_step_test_lib::model::{CompilationStepFailure, Diff};
use kahuna_compiler::testing::{PatchGraph, parse_patch_code};

pub fn verify_patch_code_parsing(
    input: &str,
    expected_graph_json: Option<&str>,
) -> Result<PatchGraph, CompilationStepFailure> {
    let result = parse_patch_code(input);

    if result.is_err() {
        return Err(CompilationStepFailure {
            error: format!("Parse error: {}", result.err().unwrap()),
            diff: None,
        });
    }

    let graph = result.unwrap();

    if let Some(expected_json_str) = expected_graph_json {
        let expected_graph: PatchGraph =
          serde_json::from_str(expected_json_str).map_err(|e| CompilationStepFailure {
            error: format!("Invalid expected JSON: {}", e),
            diff: None,
          })?;

        if graph != expected_graph {
          return Err(CompilationStepFailure {
            error: "Patch graph mismatch".to_string(),
            diff: Some(Diff {
              expected: serde_json::to_string_pretty(&expected_graph).unwrap(),
              actual: serde_json::to_string_pretty(&graph).unwrap(),
            }),
          });
        }
    }

    Ok(graph)
}
