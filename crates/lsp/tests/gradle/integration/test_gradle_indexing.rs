use tower_lsp::lsp_types::{Position, Url};

use super::fixture_builder::setup_indexed_workspace;
use lsp::features::{find_completions, find_definition};
use lsp::indexer::WorkspaceIndex;

#[tokio::test]
async fn test_android_sdk_and_bytecode_indexing() {
    let (_proj, index) = setup_indexed_workspace().await;

    assert!(
        index.name_to_decls.contains_key("Activity"),
        "Activity class should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("finish"),
        "Activity.finish() should be indexed"
    );
    assert!(
        index
            .name_to_decls
            .contains_key("getOnBackInvokedDispatcher"),
        "API 35 specific method getOnBackInvokedDispatcher should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("View"),
        "View class should be indexed"
    );

    assert!(
        index.name_to_decls.contains_key("Gson"),
        "Gson class should be indexed from bytecode jar"
    );
    assert!(
        index.name_to_decls.contains_key("toJson"),
        "Gson.toJson should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("fromJson"),
        "Gson.fromJson should be indexed"
    );
}

#[tokio::test]
async fn test_gradle_cache_version_deduplication() {
    let (_proj, index) = setup_indexed_workspace().await;

    assert!(
        index.name_to_decls.contains_key("isNewKtxVersion"),
        "core-ktx 1.15.0 symbol should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("isVisible"),
        "core-ktx 1.15.0 isVisible extension should be indexed"
    );
    assert!(
        !index.name_to_decls.contains_key("isOldKtxVersion"),
        "Older 1.12.0 symbol should NOT be indexed when 1.15.0 is present"
    );
}

#[tokio::test]
async fn test_kotlin_stdlib_and_lsp_queries() {
    let (project_dir, mut index) = setup_indexed_workspace().await;

    assert!(
        index.name_to_decls.contains_key("listOf"),
        "Kotlin stdlib listOf should be indexed"
    );
    assert!(
        index.name_to_decls.contains_key("println"),
        "Kotlin stdlib println should be indexed"
    );

    let test_file_url = Url::from_file_path(project_dir.join("MainActivity.kt")).unwrap();
    let test_code = r#"package com.example.app

import android.app.Activity
import com.google.gson.Gson

class MainActivity : Activity()

fun test(activity: MainActivity, gson: Gson) {
    activity.
    gson.
}
"#;
    index.index_source(test_file_url.clone(), test_code);

    verify_stdlib_completions(&index, &test_file_url, test_code);
    verify_stdlib_definitions(&index, &test_file_url, test_code);
}

fn verify_stdlib_completions(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let act_comps = find_completions(
        index,
        url,
        Position {
            line: 8,
            character: 13,
        },
        test_code,
    );
    let act_labels: Vec<String> = act_comps.into_iter().map(|c| c.label).collect();
    assert!(
        act_labels.contains(&"finish".to_string()),
        "Activity completion should contain finish"
    );
    assert!(act_labels.contains(&"getOnBackInvokedDispatcher".to_string()));

    let gson_comps = find_completions(
        index,
        url,
        Position {
            line: 9,
            character: 9,
        },
        test_code,
    );
    let gson_labels: Vec<String> = gson_comps.into_iter().map(|c| c.label).collect();
    assert!(
        gson_labels.contains(&"toJson".to_string()),
        "Gson completion should contain toJson"
    );
    assert!(
        gson_labels.contains(&"fromJson".to_string()),
        "Gson completion should contain fromJson"
    );
}

fn verify_stdlib_definitions(index: &WorkspaceIndex, url: &Url, test_code: &str) {
    let def_act = find_definition(
        index,
        url,
        "Activity",
        Position {
            line: 2,
            character: 20,
        },
        test_code,
    );
    assert!(
        def_act.is_some(),
        "Definition of Activity should resolve to android.jar"
    );

    let def_gson = find_definition(
        index,
        url,
        "Gson",
        Position {
            line: 3,
            character: 25,
        },
        test_code,
    );
    assert!(
        def_gson.is_some(),
        "Definition of Gson should resolve to gson-2.10.1.jar"
    );
}
