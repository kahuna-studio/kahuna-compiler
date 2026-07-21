use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
  SourceCode,
  PatchCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilationStep {
  SourceFilePreProcessing,
  SourceFileParsing,
  AstConstruction,
  AstToKahunaCodeSerialization,
  AstToJsonSerialization,
  
  PatchCodeParsing,
}

impl FromStr for CompilationStep {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.trim() {
      "source-file-pre-processing" => Ok(CompilationStep::SourceFilePreProcessing),
      "source-file-parsing" => Ok(CompilationStep::SourceFileParsing),
      "ast-to-kahuna-code-serialization" => Ok(CompilationStep::AstToKahunaCodeSerialization),
      "ast-to-json-serialization" => Ok(CompilationStep::AstToJsonSerialization),
      "patch-code-parsing" => Ok(CompilationStep::PatchCodeParsing),
      other => Err(format!("Unknown compilation step: '{other}'")),
    }
  }
}

pub struct TestCase {
  pub name: String,
  pub source: String,
  pub sections: HashMap<CompilationStep, String>,
}

pub enum CompilationStepResult {
  Success(bool),
  Failure(CompilationStepFailure),
}

#[derive(Clone, Debug)]
pub struct CompilationStepFailure {
  pub error: String,
  pub diff: Option<Diff>,
}

#[derive(Clone, Debug)]
pub struct Diff {
  pub expected: String,
  pub actual: String,
}
