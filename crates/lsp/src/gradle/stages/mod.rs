pub mod direct_deps;
pub mod fast_path;
pub mod transitive;

use std::path::Path;

use crate::gradle::android_sdk::AndroidSdkPlatform;
use crate::gradle::cache_resolver::ResolvedArtifact;

pub fn prepare_archive_paths<'a>(
    artifacts: &'a [ResolvedArtifact],
    android_platform: Option<&'a AndroidSdkPlatform>,
    prefer_sources: bool,
) -> Vec<&'a Path> {
    let mut archives = Vec::new();
    let mut seen = ahash::AHashSet::default();

    for artifact in artifacts {
        let target = artifact.target_path(prefer_sources);
        if seen.insert(target) {
            archives.push(target);
        }
    }
    if let Some(p) = android_platform {
        let sdk_jar = p.android_jar.as_path();
        if seen.insert(sdk_jar) {
            archives.push(sdk_jar);
        }
    }
    archives
}

pub fn log_resolved_artifact(artifact: &ResolvedArtifact, prefer_sources: bool) {
    let ver = artifact
        .coordinate
        .version
        .as_ref()
        .map(|v| v.raw())
        .unwrap_or("*");
    if prefer_sources && let Some(ref src) = artifact.sources_path {
        crate::logger::log!(
            "indexing",
            "Resolved sources jar: {}:{}:{} -> {}",
            artifact.coordinate.group,
            artifact.coordinate.name,
            ver,
            src.display()
        );
        return;
    }
    let kind = match artifact.binary_path.extension().and_then(|e| e.to_str()) {
        Some("aar") => "aar",
        _ => "jar",
    };
    crate::logger::log!(
        "indexing",
        "Resolved {kind}: {}:{}:{} -> {}",
        artifact.coordinate.group,
        artifact.coordinate.name,
        ver,
        artifact.binary_path.display()
    );
}
