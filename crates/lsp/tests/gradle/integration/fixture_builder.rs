use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use lsp::gradle::config::GradleConfig;
use lsp::gradle::pipeline::MultiStageIndexer;
use lsp::indexer::WorkspaceIndex;

pub fn ensure_fixtures(fixtures_dir: &Path) {
    let p35_jar = fixtures_dir
        .join("android_sdk")
        .join("platforms")
        .join("android-35")
        .join("android.jar");
    assert!(
        p35_jar.is_file(),
        "Static fixture android.jar not found at {:?}",
        p35_jar
    );
}

pub fn get_integration_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("gradle")
        .join("fixtures")
}

pub async fn setup_indexed_workspace() -> (PathBuf, WorkspaceIndex) {
    let fixtures_dir = get_integration_fixtures_dir();
    ensure_fixtures(&fixtures_dir);

    let project_dir = fixtures_dir.join("project");
    let config = GradleConfig::new()
        .with_gradle_user_home(fixtures_dir.join("gradle_cache"))
        .with_android_sdk_root(fixtures_dir.join("android_sdk"));

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let summary = MultiStageIndexer::spawn(project_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();

    assert!(summary.total_dependencies_declared >= 2);
    assert_eq!(summary.android_platform.as_deref(), Some("android-35"));
    assert_eq!(summary.kotlin_version.as_deref(), Some("2.0.20"));

    let index = Arc::try_unwrap(index).ok().unwrap().into_inner();
    (project_dir, index)
}
