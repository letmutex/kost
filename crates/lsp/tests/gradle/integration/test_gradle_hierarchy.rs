use tower_lsp::lsp_types::{Position, Url};

use super::fixture_builder::setup_indexed_workspace;
use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;

#[tokio::test]
async fn test_compose_modifier_extension_functions() {
    let (project_dir, mut index) = setup_indexed_workspace().await;

    assert!(
        index.name_to_decls.contains_key("clickable"),
        "clickable extension should be indexed from trimmed foundation-desktop sources jar"
    );
    assert!(
        index.name_to_decls.contains_key("background"),
        "background extension should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("verticalScroll"),
        "verticalScroll extension should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("horizontalScroll"),
        "horizontalScroll extension should be indexed"
    );

    let modifier_extensions = index
        .container_to_decls
        .get("Modifier")
        .expect("Modifier container decls");
    let ext_names: Vec<&str> = modifier_extensions
        .iter()
        .map(|(_, d)| d.name.as_str())
        .collect();
    assert!(
        ext_names.contains(&"clickable"),
        "Modifier container should have clickable"
    );
    assert!(
        ext_names.contains(&"background"),
        "Modifier container should have background"
    );
    assert!(
        ext_names.contains(&"verticalScroll"),
        "Modifier container should have verticalScroll"
    );

    let test_file_url = Url::from_file_path(project_dir.join("ComposeScreen.kt")).unwrap();
    let test_code = r#"package com.example.app

import androidx.compose.ui.Modifier
import androidx.compose.foundation.clickable
import androidx.compose.foundation.background

fun Screen(modifier: Modifier) {
    modifier.
}
"#;
    index.index_source(test_file_url.clone(), test_code);

    verify_modifier_completions(&index, &test_file_url, test_code);
    verify_modifier_definition(&index, &test_file_url, test_code);
}

fn verify_modifier_completions(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let completions = find_completions(
        index,
        url,
        Position {
            line: 7,
            character: 13,
        },
        test_code,
    );
    let comp_labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        comp_labels.contains(&"clickable".to_string()),
        "Completion should include clickable"
    );
    assert!(
        comp_labels.contains(&"background".to_string()),
        "Completion should include background"
    );
    assert!(
        comp_labels.contains(&"verticalScroll".to_string()),
        "Completion should include verticalScroll"
    );
}

fn verify_modifier_definition(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let def_clickable = find_definition(
        index,
        url,
        "clickable",
        Position {
            line: 3,
            character: 25,
        },
        test_code,
    );
    assert!(
        def_clickable.is_some(),
        "Definition of clickable should resolve to foundation-desktop sources jar"
    );
    let target_uri = def_clickable.unwrap().uri.to_string();
    assert!(
        target_uri.contains("foundation-desktop-1.10.2-sources.jar"),
        "Definition target URI should point to foundation-desktop-1.10.2-sources.jar: {}",
        target_uri
    );
}

#[tokio::test]
async fn test_transitive_dependency_graph_indexing() {
    let (project_dir, mut index) = setup_indexed_workspace().await;

    assert!(
        index.name_to_decls.contains_key("NonNull"),
        "Transitive dependency class NonNull from androidx.annotation should be indexed via core-ktx descriptor.bin"
    );
    assert!(
        index.name_to_decls.contains_key("CheckResult"),
        "Transitive dependency class CheckResult from androidx.annotation should be indexed via core-ktx descriptor.bin"
    );

    let test_file_url = Url::from_file_path(project_dir.join("TransitiveTest.kt")).unwrap();
    let test_code = r#"package com.example.app

import androidx.annotation.NonNull

fun getUserName(@NonNull userId: String): String {
    return userId
}
"#;
    index.index_source(test_file_url.clone(), test_code);

    verify_transitive_hover_and_def(&index, &test_file_url, test_code);
}

fn verify_transitive_hover_and_def(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let hover_non_null = find_hover(
        index,
        url,
        "NonNull",
        Position {
            line: 4,
            character: 19,
        },
        test_code,
    );
    assert!(
        hover_non_null.is_some(),
        "Hover on transitive annotation NonNull should resolve"
    );
    if let Some(h) = hover_non_null
        && let tower_lsp::lsp_types::HoverContents::Scalar(
            tower_lsp::lsp_types::MarkedString::String(text),
        ) = h.contents
    {
        assert!(
            text.contains("annotation class NonNull"),
            "Hover should include annotation declaration: {}",
            text
        );
        assert!(
            text.contains("can never be null"),
            "Hover should include KDoc documentation: {}",
            text
        );
    }

    let def_non_null = find_definition(
        index,
        url,
        "NonNull",
        Position {
            line: 4,
            character: 19,
        },
        test_code,
    );
    assert!(
        def_non_null.is_some(),
        "Definition of NonNull should resolve to transitive sources jar"
    );
    let uri = def_non_null.unwrap().uri.to_string();
    assert!(
        uri.contains("annotation-1.3.0-sources.jar"),
        "URI should point to transitive annotation sources jar: {}",
        uri
    );
}
