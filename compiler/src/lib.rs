pub mod compiler;
pub use compiler::source_file_parser::source_file_parser::parse_source_file as internal_parse_source_file;
pub use compiler::source_file_preprocessor::source_file_preprocessor::add_indentation_tags as internal_pre_process;

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
