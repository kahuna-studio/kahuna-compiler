pub mod compiler;
pub use compiler::lexer::lexer::lex as internal_parse;
pub use compiler::preprocessor::preprocessor::add_indentation_tags as internal_pre_process;
