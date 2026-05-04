use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilationStep {
  SourceFilePreProcessing,
  SourceFileParsing,
  AstConstruction,
  AstToKahunaCodeSerialization,
  AstToJsonSerialization,
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
