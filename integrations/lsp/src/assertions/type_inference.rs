use common::concat_str;
use lsp::inference::{KotlinType, TraceCollector, Tracer, infer_variable_type_traced};
use tower_lsp::lsp_types::Position;

use crate::config::TypeInferenceTest;
use crate::environment::WorkspaceEnvironment;

fn matches_kotlin_type(actual: &KotlinType, expected: &KotlinType) -> bool {
    actual == expected
}

pub fn evaluate_type_inference_test(
    environment: &WorkspaceEnvironment,
    test_case: &TypeInferenceTest,
) -> Result<(), String> {
    let file_url = environment.resolve_file_url(&test_case.file)?;
    let source_code = environment
        .get_file_source(&file_url)
        .ok_or_else(|| concat_str!("Source file not found in index: ", &test_case.file))?;

    let cursor_position = Position::new(test_case.position.line, test_case.position.col);
    let mut collector = TraceCollector::new();
    let mut tracer = Tracer::Active(&mut collector);
    let inferred_type_opt = infer_variable_type_traced(
        &environment.index,
        &file_url,
        &test_case.variable,
        cursor_position,
        &source_code,
        None,
        &mut tracer,
    );

    let Some(actual_type) = inferred_type_opt else {
        let lines: Vec<&str> = source_code.lines().collect();
        let target_line = test_case.position.line as usize;
        eprintln!(
            "=== FAILED (NONE) INFERENCE TRACE FOR {} ===\nTarget line {}: {:?}\n{}",
            test_case.name,
            target_line,
            lines.get(target_line),
            collector.render_markdown()
        );
        return Err(concat_str!(
            "Failed to infer type for variable '",
            &test_case.variable,
            "' at line ",
            &test_case.position.line.to_string(),
            ", col ",
            &test_case.position.col.to_string(),
            " in ",
            &test_case.file
        ));
    };

    let matches_expected = if let Some(expected_ast) = KotlinType::parse(&test_case.expected_type) {
        matches_kotlin_type(&actual_type, &expected_ast)
    } else {
        actual_type.to_string() == test_case.expected_type
    };

    if !matches_expected {
        let actual_type_str = actual_type.to_string();
        let lines: Vec<&str> = source_code.lines().collect();
        let target_line = test_case.position.line as usize;
        eprintln!(
            "=== MISMATCH INFERENCE TRACE FOR {} ===\nTarget line {}: {:?}\nActual: {}\nExpected: {}\n{}",
            test_case.name,
            target_line,
            lines.get(target_line),
            actual_type_str,
            test_case.expected_type,
            collector.render_markdown()
        );
        return Err(concat_str!(
            "Type mismatch for '",
            &test_case.variable,
            "': expected '",
            &test_case.expected_type,
            "', but inferred '",
            &actual_type_str,
            "'"
        ));
    }

    Ok(())
}
