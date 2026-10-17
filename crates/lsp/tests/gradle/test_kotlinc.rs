use std::sync::Arc;

use super::mock_fixtures::TestFixtureEnvironment;
use lsp::gradle::config::GradleConfig;
use lsp::gradle::kotlinc::KotlincResolver;
use lsp::gradle::pipeline::MultiStageIndexer;
use lsp::indexer::WorkspaceIndex;
use tokio::sync::RwLock;

#[test]
fn test_kotlinc_resolver_discovers_stdlib_sources_jar() {
    let env = TestFixtureEnvironment::setup();
    let kotlinc_dir = env.root_dir.join("kotlinc");
    let sources_jar = kotlinc_dir.join("lib/kotlin-stdlib-sources.jar");

    let config = GradleConfig::new().with_kotlinc_home(&kotlinc_dir);
    let artifact = KotlincResolver::find_kotlinc_stdlib(&config)
        .expect("Should resolve stdlib from kotlinc home");

    assert_eq!(artifact.coordinate.group, "org.jetbrains.kotlin");
    assert_eq!(artifact.coordinate.name, "kotlin-stdlib");
    assert_eq!(artifact.sources_path, Some(sources_jar.clone()));
    assert_eq!(artifact.binary_path, sources_jar);
}

#[tokio::test]
async fn test_non_gradle_project_resolves_kotlinc_stdlib() {
    let env = TestFixtureEnvironment::setup();
    let kotlinc_dir = env.root_dir.join("kotlinc");
    let standalone_dir = env.root_dir.join("standalone_project");

    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir)
        .with_kotlinc_home(&kotlinc_dir);

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let summary = MultiStageIndexer::spawn(standalone_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();
    let index = Arc::try_unwrap(index).ok().unwrap().into_inner();

    assert_eq!(summary.build_scripts_found, 0);
    assert_eq!(summary.catalogs_found, 0);
    assert_eq!(summary.android_platform, None);
    assert!(
        index.name_to_decls.contains_key("listOf"),
        "kotlinc stdlib symbols should be indexed for standalone project"
    );
}

#[tokio::test]
async fn test_non_gradle_project_without_kotlinc_indexes_zero_symbols() {
    let env = TestFixtureEnvironment::setup();
    let standalone_dir = env.root_dir.join("standalone_project");
    let empty_kotlinc_dir = env.root_dir.join("non_existent_kotlinc");

    let config = GradleConfig::new()
        .with_gradle_user_home(&env.gradle_home)
        .with_android_sdk_root(&env.sdk_dir)
        .with_kotlinc_home(&empty_kotlinc_dir);

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let summary = MultiStageIndexer::spawn(standalone_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();
    let index = Arc::try_unwrap(index).ok().unwrap().into_inner();

    assert_eq!(summary.build_scripts_found, 0);
    assert_eq!(summary.catalogs_found, 0);
    assert_eq!(summary.android_platform, None);
    assert_eq!(
        summary.total_symbols_indexed, 0,
        "No symbols should be indexed when there are no Gradle files and no kotlinc"
    );
    assert!(index.name_to_decls.is_empty());
}
