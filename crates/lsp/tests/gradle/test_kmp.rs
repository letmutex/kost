use std::fs;
use std::path::{Path, PathBuf};

use lsp::gradle::cache_resolver::GradleCacheResolver;
use lsp::gradle::dependency::{DependencyCoordinate, KmpTarget, deduplicate_newest};

struct TempTestDir(PathBuf);

impl TempTestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "kotlin_lsp_kmp_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_kmp_target_extraction_and_ordering() {
    let (base_desktop, target_desktop) = KmpTarget::from_artifact_name("material3-desktop");
    assert_eq!(base_desktop, "material3");
    assert_eq!(target_desktop, KmpTarget::Desktop);

    let (base_jvm, target_jvm) = KmpTarget::from_artifact_name("material3-jvm");
    assert_eq!(base_jvm, "material3");
    assert_eq!(target_jvm, KmpTarget::Jvm);

    let (base_none, target_none) = KmpTarget::from_artifact_name("material3");
    assert_eq!(base_none, "material3");
    assert_eq!(target_none, KmpTarget::None);

    // Verify ordering: Desktop > Jvm > None
    assert!(KmpTarget::Desktop > KmpTarget::Jvm);
    assert!(KmpTarget::Jvm > KmpTarget::None);
    assert!(KmpTarget::Desktop > KmpTarget::None);

    // Standalone names without hyphen prefix should not be stripped
    let (base_d, target_d) = KmpTarget::from_artifact_name("desktop");
    assert_eq!(base_d, "desktop");
    assert_eq!(target_d, KmpTarget::None);

    let (base_j, target_j) = KmpTarget::from_artifact_name("jvm");
    assert_eq!(base_j, "jvm");
    assert_eq!(target_j, KmpTarget::None);
}

#[test]
fn test_kmp_candidate_names_preference_order() {
    let dep =
        DependencyCoordinate::parse("org.jetbrains.compose.material3:material3:1.7.0").unwrap();
    let candidates = dep.kmp_candidate_names();
    assert_eq!(
        candidates,
        vec![
            "material3-desktop".to_string(),
            "material3-jvm".to_string(),
            "material3".to_string(),
        ]
    );

    let dep_jvm = DependencyCoordinate::parse("io.ktor:ktor-client-core-jvm:3.0.0").unwrap();
    let candidates_jvm = dep_jvm.kmp_candidate_names();
    assert_eq!(
        candidates_jvm,
        vec![
            "ktor-client-core-desktop".to_string(),
            "ktor-client-core-jvm".to_string(),
            "ktor-client-core".to_string(),
        ]
    );
}

#[test]
fn test_kmp_deduplicate_newest_preference() {
    // When versions are identical, -desktop > -jvm > no-suffix
    let deps = vec![
        DependencyCoordinate::parse("org.jetbrains.compose.ui:ui:1.7.0").unwrap(),
        DependencyCoordinate::parse("org.jetbrains.compose.ui:ui-jvm:1.7.0").unwrap(),
        DependencyCoordinate::parse("org.jetbrains.compose.ui:ui-desktop:1.7.0").unwrap(),
    ];
    let deduped = deduplicate_newest(deps);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].name, "ui-desktop");

    // -jvm > no-suffix
    let deps_jvm_wins = vec![
        DependencyCoordinate::parse("io.ktor:ktor-client-core:3.0.0").unwrap(),
        DependencyCoordinate::parse("io.ktor:ktor-client-core-jvm:3.0.0").unwrap(),
    ];
    let deduped_jvm = deduplicate_newest(deps_jvm_wins);
    assert_eq!(deduped_jvm.len(), 1);
    assert_eq!(deduped_jvm[0].name, "ktor-client-core-jvm");

    // Newer version always wins regardless of target
    let deps_version_wins = vec![
        DependencyCoordinate::parse("org.jetbrains.compose.ui:ui-desktop:1.6.0").unwrap(),
        DependencyCoordinate::parse("org.jetbrains.compose.ui:ui-jvm:1.7.0").unwrap(),
    ];
    let deduped_ver = deduplicate_newest(deps_version_wins);
    assert_eq!(deduped_ver.len(), 1);
    assert_eq!(deduped_ver[0].version.as_ref().unwrap().raw(), "1.7.0");
    assert_eq!(deduped_ver[0].name, "ui-jvm");
}

fn create_mock_jar(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn test_kmp_cache_resolver_preference_order() {
    let tmp = TempTestDir::new();
    let cache_dir = tmp.path().join("caches/modules-2/files-2.1");
    let group = "org.jetbrains.compose.material3";

    // Create desktop variant
    let desktop_jar = cache_dir
        .join(group)
        .join("material3-desktop/1.7.0/hash_d/material3-desktop-1.7.0.jar");
    create_mock_jar(&desktop_jar, "desktop content");

    // Create jvm variant
    let jvm_jar = cache_dir
        .join(group)
        .join("material3-jvm/1.7.0/hash_j/material3-jvm-1.7.0.jar");
    create_mock_jar(&jvm_jar, "jvm content");

    // Create base variant
    let base_jar = cache_dir
        .join(group)
        .join("material3/1.7.0/hash_b/material3-1.7.0.jar");
    create_mock_jar(&base_jar, "base content");

    // Query for base coordinate "material3" should prefer desktop
    let dep_base =
        DependencyCoordinate::parse("org.jetbrains.compose.material3:material3:1.7.0").unwrap();
    let resolved =
        GradleCacheResolver::resolve_single(&dep_base, Some(tmp.path()), None, &[]).unwrap();
    assert_eq!(resolved.coordinate.name, "material3-desktop");
    assert_eq!(resolved.binary_path, desktop_jar);

    // If desktop is removed, jvm should be preferred next
    fs::remove_file(&desktop_jar).unwrap();
    let resolved_jvm =
        GradleCacheResolver::resolve_single(&dep_base, Some(tmp.path()), None, &[]).unwrap();
    assert_eq!(resolved_jvm.coordinate.name, "material3-jvm");
    assert_eq!(resolved_jvm.binary_path, jvm_jar);

    // If jvm is removed, base should be resolved
    fs::remove_file(&jvm_jar).unwrap();
    let resolved_base =
        GradleCacheResolver::resolve_single(&dep_base, Some(tmp.path()), None, &[]).unwrap();
    assert_eq!(resolved_base.coordinate.name, "material3");
    assert_eq!(resolved_base.binary_path, base_jar);
}
