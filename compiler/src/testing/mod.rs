//! This module exports all functions and types needed for testing the compiler.
//! This is a test-specific API and should not be relied upon for production code.

pub use crate::ast::basics::File;
pub use crate::compilation_steps::ast_construction::ast_mapper::map_to_ast;
pub use crate::compilation_steps::source_file_parser::source_file_parser::parse_source_file;
pub use crate::compilation_steps::source_file_preprocessor::source_file_preprocessor::add_indentation_tags;
pub use crate::compilation_steps::source_file_parser::source_file_parser::Rule;
pub use crate::compilation_steps::ast_to_json_serializer::ast_to_json_serializer::AstJsonSerializer;
pub use crate::compilation_steps::ast_to_kahuna_code_serializer::ast_to_kahuna_code::{AstPrinter, AstVisitor};
