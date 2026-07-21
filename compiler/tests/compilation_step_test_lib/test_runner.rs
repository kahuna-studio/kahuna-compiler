use std::collections::HashMap;
use std::fs;
use indexmap::IndexMap;
use crate::compilation_step_test_lib::model::{CompilationStep, CompilationStepFailure, CompilationStepResult, FileType, TestCase};
use crate::compilation_step_test_lib::steps::ast_construction_tests::verify_ast_construction;
use crate::compilation_step_test_lib::steps::ast_to_json_serializer_tests::verify_ast_mapping_by_json;
use crate::compilation_step_test_lib::steps::ast_to_kahuna_code_serializer_tests::verify_ast_mapping_by_kahuna_code;
use crate::compilation_step_test_lib::steps::patch_code_parser_tests::verify_patch_code_parsing;
use crate::compilation_step_test_lib::steps::source_file_parser_tests::verify_source_file_parsing;
use crate::compilation_step_test_lib::steps::source_file_preprocessor_tests::verify_source_file_pre_processing;

pub fn run_source_code_test_case(test_case: &TestCase) -> IndexMap<CompilationStep, CompilationStepResult> {
    let mut results: IndexMap<CompilationStep, CompilationStepResult> = IndexMap::new();

    // ----------------------- Pre-processing -----------------------

    let step = CompilationStep::SourceFilePreProcessing;
    let source = test_case.source.clone();
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_source_file_pre_processing(&source, expected);

    update_results(
        &mut results,
        result.clone().map(|_| ()),
        step,
        expected.is_some(),
    );
    if result.is_err() {
        return results;
    }

    // ----------------------- Parsing -----------------------

    let step = CompilationStep::SourceFileParsing;
    let source = result.unwrap();
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_source_file_parsing(source.as_str(), expected);

    update_results(
        &mut results,
        result.clone().map(|_| ()),
        step,
        expected.is_some(),
    );
    if result.is_err() {
        return results;
    }

    // ----------------------- AST construction -----------------------

    let step = CompilationStep::AstConstruction;
    let source = result.unwrap();

    let result = verify_ast_construction(&mut source.clone());

    update_results(
        &mut results,
        result.clone().map(|_| ()),
        step,
        expected.is_some(),
    );
    if result.is_err() {
        return results;
    }
    let ast = result.unwrap();

    // ----------------------- AST mapping tested via Kahuna code -----------------------

    let step = CompilationStep::AstToKahunaCodeSerialization;
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_ast_mapping_by_kahuna_code(&mut ast.clone(), expected);

    update_results(
        &mut results,
        result.clone().map(|_| ()),
        step,
        expected.is_some(),
    );
    if result.is_err() {
        return results;
    }

    // ----------------------- AST mapping tested via JSON -----------------------

    let step = CompilationStep::AstToJsonSerialization;
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_ast_mapping_by_json(&mut ast.clone(), expected);

    update_results(
        &mut results,
        result.clone().map(|_| ()),
        step,
        expected.is_some(),
    );
    if result.is_err() {
        return results;
    }

    results
}


pub fn run_patch_code_test_case(test_case: &TestCase) -> IndexMap<CompilationStep, CompilationStepResult> {
    let mut results: IndexMap<CompilationStep, CompilationStepResult> = IndexMap::new();

    // ----------------------- Parsing -----------------------

    let step = CompilationStep::PatchCodeParsing;
    let source = test_case.source.clone();
    let expected = test_case.sections.get(&step).map(|s| s.as_str());

    let result = verify_patch_code_parsing(&source, expected);

    update_results(
        &mut results,
        result.clone().map(|_| ()),
        step,
        expected.is_some(),
    );
    if result.is_err() {
        return results;
    }

    results
}


pub fn load_cases(file_type: FileType, dir: &str) -> Vec<TestCase> {
    let mut cases = Vec::new();

    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("test") {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap();
        cases.push(parse_case(
            path.file_stem().unwrap().to_string_lossy().to_string(),
            &content,
        ));
    }

    cases
}

fn parse_case(name: String, content: &str) -> TestCase {
    let mut parts = content.split("\n===");
    let source = parts.next().unwrap().trim().to_string();

    let mut sections: HashMap<CompilationStep, String> = HashMap::new();

    for part in parts {
        let mut lines = part.trim().lines();
        let key = lines.next().unwrap().trim().to_string();
        let compilation_step = key
            .parse::<CompilationStep>()
            .unwrap_or_else(|_| panic!("Invalid compilation step '{key}' in test case '{name}'"));
        let value = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        sections.insert(compilation_step, value);
    }

    TestCase {
        name,
        source,
        sections,
    }
}

fn update_results(
    results: &mut IndexMap<CompilationStep, CompilationStepResult>,
    result: Result<(), CompilationStepFailure>,
    compilation_step: CompilationStep,
    has_spec: bool,
) {
    results.insert(
        compilation_step,
        match result {
            Ok(_) => CompilationStepResult::Success(has_spec),
            Err(failure) => CompilationStepResult::Failure(failure),
        },
    );
}
