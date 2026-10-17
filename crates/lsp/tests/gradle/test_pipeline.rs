use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{Position, Url};

use super::mock_fixtures::TestFixtureEnvironment;
use lsp::features::{find_completions, find_definition};
use lsp::gradle::config::GradleConfig;
use lsp::gradle::pipeline::MultiStageIndexer;
use lsp::gradle::stages::direct_deps::run_direct_deps_stage;
use lsp::gradle::stages::fast_path::run_fast_path_stage;
use lsp::gradle::stages::transitive::run_transitive_stage;
use lsp::indexer::WorkspaceIndex;

#[tokio::test]
async fn test_multi_stage_indexing_summary_and_deduplication() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir);

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let summary = MultiStageIndexer::spawn(env.project_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();
    let index = Arc::try_unwrap(index).ok().unwrap().into_inner();

    // Verify multi-stage summary stats
    assert!(summary.catalogs_found >= 1, "Catalog should be discovered");
    assert!(
        summary.build_scripts_found >= 2,
        "Build scripts should be discovered"
    );
    assert_eq!(summary.android_platform.as_deref(), Some("android-35"));
    assert!(
        summary.android_sdk_jar.is_some(),
        "Resolved android sdk jar should be recorded"
    );
    assert_eq!(summary.kotlin_version.as_deref(), Some("2.0.20"));
    assert!(
        summary.total_symbols_indexed > 0,
        "Symbols should be indexed from archives"
    );
    assert!(
        !summary.resolved_artifacts.is_empty(),
        "Resolved artifacts should be recorded"
    );
    assert!(
        summary.total_archives_indexed() > 0,
        "Total archives indexed should be greater than zero"
    );
    assert_eq!(
        summary.total_archives_indexed(),
        summary.jar_archives_indexed() + summary.aar_archives_indexed(),
        "Total archives should equal jar + aar archives"
    );
    assert_eq!(
        summary.unique_dependencies_indexed,
        summary.resolved_artifacts.len(),
        "Unique dependencies indexed must match resolved artifacts count"
    );
    let display = summary.summary_display_line();
    assert!(
        display.contains("Indexed"),
        "Summary display line must contain archive info: {display}"
    );
    assert!(
        display.contains("archives"),
        "Summary display line must report archive count: {display}"
    );

    // Verify newest version deduplication: 1.15.0 vs 1.12.0
    assert!(
        index.name_to_decls.contains_key("isNewKtxVersion"),
        "core-ktx 1.15.0 symbol 'isNewKtxVersion' should be indexed"
    );
    assert!(
        !index.name_to_decls.contains_key("isOldKtxVersion"),
        "Older version 1.12.0 symbol 'isOldKtxVersion' should not be indexed"
    );

    // Verify Android SDK symbols from API 35 & Stdlib
    assert!(index.name_to_decls.contains_key("Activity"));
    assert!(
        index
            .name_to_decls
            .contains_key("getOnBackInvokedDispatcher")
    );
    assert!(index.name_to_decls.contains_key("listOf"));
}

#[tokio::test]
async fn test_multi_stage_indexing_lsp_symbol_resolution() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir);

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let _ = MultiStageIndexer::spawn(env.project_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();
    let mut index = Arc::try_unwrap(index).ok().unwrap().into_inner();

    let test_file_url =
        Url::from_file_path(env.project_dir.join("app").join("MainActivity.kt")).unwrap();
    let test_source = r#"package com.example.app

import android.app.Activity

class MainActivity : Activity()

fun test(activity: MainActivity) {
    activity.
}
"#;
    index.index_source(test_file_url.clone(), test_source);

    // Test member completion for Activity inherited members
    let completions = find_completions(
        &index,
        &test_file_url,
        Position {
            line: 7,
            character: 13,
        },
        test_source,
    );
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"finish".to_string()),
        "Should complete 'finish'"
    );
    assert!(
        labels.contains(&"getOnBackInvokedDispatcher".to_string()),
        "Should complete 'getOnBackInvokedDispatcher'"
    );

    // Test definition navigation to Android SDK Activity
    let def = find_definition(
        &index,
        &test_file_url,
        "Activity",
        Position {
            line: 2,
            character: 20,
        },
        test_source,
    );
    assert!(
        def.is_some(),
        "Should resolve definition for Activity from android.jar"
    );
}

#[test]
fn test_progressive_stages_isolation() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir);

    let mut index = WorkspaceIndex::new();
    let mut indexed_keys = ahash::AHashSet::default();
    let mut processed_archives = ahash::AHashSet::default();

    // Stage 1: Fast Path only (Stdlib + Android SDK)
    let fast_path = run_fast_path_stage(&env.project_dir, &config, Some("2.0.20"), Some(35), true);
    for (url, syms) in fast_path.symbols {
        index.insert_file_symbols(url, syms);
    }
    if let Some(ref p) = fast_path.android_platform {
        processed_archives.insert(p.android_jar.clone());
    }
    if let Some(ref stdlib) = fast_path.stdlib_artifact {
        indexed_keys.insert((
            stdlib.coordinate.group.clone(),
            stdlib.coordinate.name.clone(),
        ));
        stdlib.record_processed(&mut processed_archives, config.prefer_sources);
    }

    assert!(
        index.name_to_decls.contains_key("listOf"),
        "Stdlib symbols must be available immediately after Stage 1"
    );
    assert!(
        index.name_to_decls.contains_key("Activity"),
        "Android SDK symbols must be available immediately after Stage 1"
    );
    assert!(
        !index.name_to_decls.contains_key("isNewKtxVersion"),
        "Direct deps symbols should not yet be in index before Stage 2"
    );

    // Stage 2: Direct Dependencies
    let (catalogs, scripts) = lsp::gradle::discovery::discover_build_files(&env.project_dir);
    let (all_deps, _, _) =
        lsp::gradle::discovery::collect_declared_dependencies(&catalogs, &scripts);
    let deduped = lsp::gradle::dependency::deduplicate_newest(all_deps);

    let direct_res = run_direct_deps_stage(&deduped, &config, &indexed_keys, &processed_archives);
    for (url, syms) in direct_res.symbols {
        index.insert_file_symbols(url, syms);
    }
    for art in &direct_res.resolved_artifacts {
        indexed_keys.insert((art.coordinate.group.clone(), art.coordinate.name.clone()));
        art.record_processed(&mut processed_archives, config.prefer_sources);
    }

    assert!(
        index.name_to_decls.contains_key("isNewKtxVersion"),
        "Direct deps symbols must be available after Stage 2"
    );

    // Stage 3: Transitive Delta
    let trans_res = run_transitive_stage(&deduped, &config, &indexed_keys, &processed_archives);
    for (url, syms) in trans_res.symbols {
        index.insert_file_symbols(url, syms);
    }
}

#[tokio::test]
async fn test_async_spawn_pipeline_streaming() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir);

    let shared_index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let handle =
        MultiStageIndexer::spawn(env.project_dir.clone(), config, Arc::clone(&shared_index));

    handle.await.unwrap();

    let index = shared_index.read().await;
    assert!(index.name_to_decls.contains_key("listOf"));
    assert!(index.name_to_decls.contains_key("Activity"));
    assert!(index.name_to_decls.contains_key("isNewKtxVersion"));
}

#[test]
fn test_duplicate_resolved_archives_skipped_across_stages() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir);

    let (catalogs, scripts) = lsp::gradle::discovery::discover_build_files(&env.project_dir);
    let (all_deps, _, _) =
        lsp::gradle::discovery::collect_declared_dependencies(&catalogs, &scripts);
    let deduped = lsp::gradle::dependency::deduplicate_newest(all_deps);

    let indexed_keys = ahash::AHashSet::default();
    let mut processed_archives = ahash::AHashSet::default();

    // Stage 2 resolves and records direct dependencies
    let direct_res = run_direct_deps_stage(&deduped, &config, &indexed_keys, &processed_archives);
    for art in &direct_res.resolved_artifacts {
        art.record_processed(&mut processed_archives, config.prefer_sources);
    }
    assert!(!direct_res.resolved_artifacts.is_empty());

    // Stage 3 runs with an empty indexed_keys (simulating key mismatch),
    // but with processed_archives populated from stage 2.
    // Transitive stage must filter out any artifact pointing to the already-processed archives.
    let trans_res = run_transitive_stage(&deduped, &config, &indexed_keys, &processed_archives);
    for art in &trans_res.delta_artifacts {
        let target = art.target_path(config.prefer_sources);
        assert!(
            !processed_archives
                .contains(&lsp::gradle::cache_resolver::normalize_archive_path(target)),
            "Stage 3 must not re-process already indexed archive: {:?}",
            target
        );
    }
}

#[tokio::test]
async fn test_pure_jvm_project_skips_android_jar_indexing() {
    let env = TestFixtureEnvironment::setup();
    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir);

    // Use static JVM project without android {} block
    let jvm_proj_dir = env.root_dir.join("jvm_project");

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let summary = MultiStageIndexer::spawn(jvm_proj_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();
    let index = Arc::try_unwrap(index).ok().unwrap().into_inner();

    assert_eq!(
        summary.android_platform, None,
        "Android SDK platform must not be resolved for pure JVM project"
    );
    assert_eq!(
        summary.android_sdk_jar, None,
        "Android SDK jar must not be indexed for pure JVM project"
    );
    assert!(
        !index.name_to_decls.contains_key("Activity"),
        "Activity from android.jar must not exist in index for pure JVM project"
    );
    assert!(
        index.name_to_decls.contains_key("listOf"),
        "Kotlin stdlib symbols should still be indexed"
    );
}
