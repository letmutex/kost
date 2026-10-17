use tower_lsp::lsp_types::{Position, Url};

use super::fixture_builder::setup_indexed_workspace;
use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;

#[tokio::test]
async fn test_external_constructor_parameter_completion_and_hover() {
    let (project_dir, mut index) = setup_indexed_workspace().await;
    let test_file_url = Url::from_file_path(project_dir.join("ConstructorTest.kt")).unwrap();
    let test_code = r#"package com.example.app

import kotlin.Pair

fun testConstructorCompletion() {
    val p = Pair(
}

fun testConstructorHover() {
    val p = Pair(first = 42, second = "hello")
}
"#;
    index.index_source(test_file_url.clone(), test_code);

    let completions = find_completions(
        &index,
        &test_file_url,
        Position {
            line: 5,
            character: 17,
        },
        test_code,
    );
    let comp_labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        comp_labels.contains(&"first".to_string()),
        "Pair constructor should complete 'first'"
    );
    assert!(
        comp_labels.contains(&"second".to_string()),
        "Pair constructor should complete 'second'"
    );

    verify_constructor_hover(&index, &test_file_url, test_code);
    verify_constructor_def(&index, &test_file_url, test_code);
}

fn verify_constructor_hover(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let hover_first = find_hover(
        index,
        url,
        "first",
        Position {
            line: 9,
            character: 19,
        },
        test_code,
    );
    assert!(
        hover_first.is_some(),
        "Hover on external constructor parameter 'first' should resolve"
    );
    if let Some(h) = hover_first
        && let tower_lsp::lsp_types::HoverContents::Scalar(
            tower_lsp::lsp_types::MarkedString::String(text),
        ) = h.contents
    {
        assert!(
            text.contains("first"),
            "Hover should include parameter signature: {}",
            text
        );
        assert!(
            text.contains("The first value."),
            "Hover should include param doc: {}",
            text
        );
    }
}

fn verify_constructor_def(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let def_second = find_definition(
        index,
        url,
        "second",
        Position {
            line: 9,
            character: 31,
        },
        test_code,
    );
    assert!(
        def_second.is_some(),
        "Definition of 'second' should resolve to stdlib sources jar"
    );
    let uri = def_second.unwrap().uri.to_string();
    assert!(
        uri.contains("kotlin-stdlib-2.0.20-sources.jar"),
        "URI should point to stdlib sources: {}",
        uri
    );
}

#[tokio::test]
async fn test_external_function_parameter_completion_and_hover() {
    let (project_dir, mut index) = setup_indexed_workspace().await;
    let test_file_url = Url::from_file_path(project_dir.join("FunctionParamTest.kt")).unwrap();
    let test_code = r#"package com.example.app

import androidx.compose.ui.Modifier
import androidx.compose.foundation.clickable

fun testFunctionParamCompletion() {
    Modifier.clickable(
}

fun testFunctionParamHover() {
    Modifier.clickable(enabled = false)
}
"#;
    index.index_source(test_file_url.clone(), test_code);

    let completions = find_completions(
        &index,
        &test_file_url,
        Position {
            line: 6,
            character: 23,
        },
        test_code,
    );
    let comp_labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        comp_labels.contains(&"enabled".to_string()),
        "clickable function should complete 'enabled'"
    );
    assert!(
        comp_labels.contains(&"onClick".to_string()),
        "clickable function should complete 'onClick'"
    );

    verify_function_param_hover(&index, &test_file_url, test_code);
    verify_function_param_def(&index, &test_file_url, test_code);
}

fn verify_function_param_hover(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let hover_enabled = find_hover(
        index,
        url,
        "enabled",
        Position {
            line: 10,
            character: 25,
        },
        test_code,
    );
    assert!(
        hover_enabled.is_some(),
        "Hover on external function parameter 'enabled' should resolve"
    );
    if let Some(h) = hover_enabled
        && let tower_lsp::lsp_types::HoverContents::Scalar(
            tower_lsp::lsp_types::MarkedString::String(text),
        ) = h.contents
    {
        assert!(
            text.contains("enabled"),
            "Hover should include parameter signature: {}",
            text
        );
    }
}

fn verify_function_param_def(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let def_enabled = find_definition(
        index,
        url,
        "enabled",
        Position {
            line: 10,
            character: 25,
        },
        test_code,
    );
    assert!(
        def_enabled.is_some(),
        "Definition of 'enabled' should resolve to foundation-desktop sources jar"
    );
    let uri = def_enabled.unwrap().uri.to_string();
    assert!(
        uri.contains("foundation-desktop-1.10.2-sources.jar"),
        "URI should point to foundation sources: {}",
        uri
    );
}
