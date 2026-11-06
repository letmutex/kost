use common::concat_str;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::Position;

use crate::config::TypeInferenceTest;
use crate::environment::WorkspaceEnvironment;

fn matches_kotlin_type(actual: &KotlinType, expected: &KotlinType) -> bool {
    if actual == expected {
        return true;
    }

    match (actual, expected) {
        (
            KotlinType::Class {
                name: actual_name,
                args: actual_args,
            },
            KotlinType::Class {
                name: expected_name,
                args: expected_args,
            },
        ) => {
            let names_match = actual_name == expected_name
                || actual_name.ends_with(&concat_str!(".", expected_name.as_str()))
                || expected_name.ends_with(&concat_str!(".", actual_name.as_str()));
            names_match && actual_args == expected_args
        }
        (KotlinType::Nullable(actual_inner), KotlinType::Nullable(expected_inner)) => {
            matches_kotlin_type(actual_inner, expected_inner)
        }
        _ => false,
    }
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
    let inferred_type_opt = infer_variable_type(
        &environment.index,
        &file_url,
        &test_case.variable,
        cursor_position,
        &source_code,
        None,
    );

    let Some(actual_type) = inferred_type_opt else {
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
        let actual_type_str = actual_type.to_string();
        actual_type_str == test_case.expected_type
            || actual_type_str.ends_with(&concat_str!(".", &test_case.expected_type))
            || test_case
                .expected_type
                .ends_with(&concat_str!(".", &actual_type_str))
    };

    if !matches_expected {
        let actual_type_str = actual_type.to_string();
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
