use common::concat_str;
use resolver::coordinate::DependencyCoordinate;
use resolver::maven_metadata::parse_maven_metadata;
use resolver::repo::{Repository, RepositoryRegistry};
use resolver::version::ArtifactVersion;

use crate::cache::GradleCache;
use crate::downloader::Downloader;

/// Probe a single repository for `maven-metadata.xml`.
async fn probe_repo_maven_metadata(
    downloader: &Downloader,
    repo: &Repository,
    group: &str,
    artifact: &str,
) -> Option<resolver::maven_metadata::MavenMetadata> {
    if downloader.is_repo_unreachable(&repo.url).await {
        return None;
    }

    let url = repo.maven_metadata_url(group, artifact);
    match downloader.fetch_text_internal(&url).await {
        Ok(content) => {
            downloader.record_connection_success(&repo.url).await;
            if let Ok(meta) = parse_maven_metadata(&content) {
                return Some(meta);
            }
        }
        Err(err) => {
            downloader.record_probe_error(&repo.url, &err).await;
        }
    }
    None
}

pub use resolver::version::select_best_version;

/// Resolve the latest version for a dependency coordinate across registered repositories and local cache.
pub async fn resolve_latest_version(
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    coord: &DependencyCoordinate,
    cache: &GradleCache,
    offline: bool,
) -> Result<ArtifactVersion, String> {
    if !offline && !registry.is_empty() {
        let candidates = coord.kmp_probe_candidates();
        let group = coord.group.clone();
        for cand_name in &candidates {
            let artifact = smol_str::SmolStr::new(cand_name);
            let mut join_set = tokio::task::JoinSet::new();
            for repo in &registry.repositories {
                let downloader_clone = downloader.clone();
                let repo_clone = repo.clone();
                let group_clone = group.clone();
                let artifact_clone = artifact.clone();
                join_set.spawn(async move {
                    probe_repo_maven_metadata(
                        &downloader_clone,
                        &repo_clone,
                        &group_clone,
                        &artifact_clone,
                    )
                    .await
                });
            }

            let mut all_versions = Vec::new();
            while let Some(res) = join_set.join_next().await {
                if let Ok(Some(meta)) = res {
                    all_versions.extend(meta.versions);
                    if let Some(rel) = meta.release {
                        all_versions.push(rel);
                    }
                    if let Some(lat) = meta.latest {
                        all_versions.push(lat);
                    }
                }
            }

            if let Some(best) = select_best_version(&all_versions) {
                return Ok(best);
            }
        }
    }

    // Fall back to local Gradle cache and M2 cache inspection
    let candidates = coord.kmp_probe_candidates();
    for cand_name in &candidates {
        let mut cached = cache.find_cached_versions(&coord.group, cand_name);
        cached.extend(cache.find_m2_cached_versions(&coord.group, cand_name));
        if let Some(best) = select_best_version(&cached) {
            return Ok(best);
        }
    }

    if offline {
        Err(concat_str!(
            "Offline mode: no cached versions found for '",
            &coord.group,
            ":",
            &coord.name,
            "' in local Gradle or M2 cache."
        ))
    } else {
        Err(concat_str!(
            "Could not resolve latest version for '",
            &coord.group,
            ":",
            &coord.name,
            "'. No versions found in configured repositories or local cache."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_best_version_prefers_stable_over_prerelease() {
        let versions = vec![
            ArtifactVersion::parse("1.2.0"),
            ArtifactVersion::parse("1.3.0-rc01"),
            ArtifactVersion::parse("1.3.0-beta02"),
            ArtifactVersion::parse("1.1.0"),
        ];

        let selected = select_best_version(&versions).unwrap();
        assert_eq!(selected.raw(), "1.2.0");
    }

    #[test]
    fn test_select_best_version_falls_back_to_latest_prerelease() {
        let versions = vec![
            ArtifactVersion::parse("0.1.0-alpha01"),
            ArtifactVersion::parse("0.2.0-beta02"),
            ArtifactVersion::parse("0.2.0-alpha01"),
        ];

        let selected = select_best_version(&versions).unwrap();
        assert_eq!(selected.raw(), "0.2.0-beta02");
    }

    #[test]
    fn test_select_best_version_numerical_precedence() {
        let versions = vec![
            ArtifactVersion::parse("1.9.0"),
            ArtifactVersion::parse("1.10.1"),
            ArtifactVersion::parse("1.2.0"),
        ];

        let selected = select_best_version(&versions).unwrap();
        assert_eq!(selected.raw(), "1.10.1");
    }
}
