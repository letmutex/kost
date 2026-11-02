pub use resolver::graph::resolve_offline_transitives;
use resolver::graph::try_resolve_cached_transitives;

use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::module::GradleModuleMetadata;
use crate::pom::PomModel;
use crate::repo::{Repository, RepositoryRegistry};

pub mod parent;

use parent::resolve_and_inherit_parent_poms;

/// Resolve metadata for a dependency coordinate across repositories.
pub async fn resolve_metadata(
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    coord: &DependencyCoordinate,
    cache: &GradleCache,
    offline: bool,
) -> Result<Vec<DependencyCoordinate>, String> {
    if coord.version.is_none() {
        return Ok(Vec::new());
    }

    if let Some(in_mem) = cache.get_resolved(coord) {
        return Ok(in_mem);
    }

    if let Some(cached_deps) = try_resolve_cached_transitives(coord, cache) {
        cache.insert_resolved(coord.clone(), cached_deps.clone());
        return Ok(cached_deps);
    }

    if offline {
        return Ok(Vec::new());
    }

    let candidates = coord.kmp_probe_candidates();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let mut primary_target = coord.clone();
    primary_target.name = smol_str::SmolStr::new(&candidates[0]);
    if let Some(deps) = probe_remote_metadata(downloader, registry, &primary_target, cache).await {
        cache.insert_resolved(coord.clone(), deps.clone());
        return Ok(deps);
    }

    if let Some(deps) =
        probe_candidate_variants(downloader, registry, coord, &candidates, cache).await
    {
        cache.insert_resolved(coord.clone(), deps.clone());
        return Ok(deps);
    }

    Ok(Vec::new())
}

async fn probe_candidate_variants(
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    coord: &DependencyCoordinate,
    candidates: &[String],
    cache: &GradleCache,
) -> Option<Vec<DependencyCoordinate>> {
    if candidates.len() <= 1 {
        return None;
    }

    let mut join_set = tokio::task::JoinSet::new();
    let variant_count = candidates.len() - 1;
    for (idx, cand_name) in candidates.iter().enumerate().skip(1) {
        let downloader_clone = downloader.clone();
        let registry_clone = registry.clone();
        let mut target = coord.clone();
        target.name = smol_str::SmolStr::new(cand_name);
        let cache_clone = cache.clone();
        let relative_idx = idx - 1;
        join_set.spawn(async move {
            let res =
                probe_remote_metadata(&downloader_clone, &registry_clone, &target, &cache_clone)
                    .await;
            (relative_idx, res)
        });
    }

    let mut results = vec![None; variant_count];
    while let Some(res) = join_set.join_next().await {
        if let Ok((relative_idx, outcome)) = res {
            results[relative_idx] = Some(outcome);
            if let Some(winner) = evaluate_earliest_resolved(&results) {
                join_set.abort_all();
                return winner;
            }
        }
    }

    evaluate_earliest_resolved(&results).flatten()
}

async fn probe_remote_metadata(
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    coord: &DependencyCoordinate,
    cache: &GradleCache,
) -> Option<Vec<DependencyCoordinate>> {
    let repos = &registry.repositories;
    if repos.is_empty() {
        return None;
    }
    if repos.len() == 1 {
        return probe_repo_metadata(downloader, &repos[0], registry, coord, cache).await;
    }

    let mut join_set = tokio::task::JoinSet::new();
    for (idx, repo) in repos.iter().enumerate() {
        let downloader_clone = downloader.clone();
        let repo_clone = repo.clone();
        let registry_clone = registry.clone();
        let coord_clone = coord.clone();
        let cache_clone = cache.clone();
        join_set.spawn(async move {
            let res = probe_repo_metadata(
                &downloader_clone,
                &repo_clone,
                &registry_clone,
                &coord_clone,
                &cache_clone,
            )
            .await;
            (idx, res)
        });
    }

    let mut results = vec![None; repos.len()];
    while let Some(join_res) = join_set.join_next().await {
        if let Ok((idx, outcome)) = join_res {
            results[idx] = Some(outcome);
            if let Some(winner) = evaluate_earliest_resolved(&results) {
                join_set.abort_all();
                return winner;
            }
        }
    }

    evaluate_earliest_resolved(&results).flatten()
}

async fn probe_repo_metadata(
    downloader: &Downloader,
    repo: &Repository,
    registry: &RepositoryRegistry,
    coord: &DependencyCoordinate,
    cache: &GradleCache,
) -> Option<Vec<DependencyCoordinate>> {
    if downloader.is_repo_unreachable(&repo.url).await {
        return None;
    }

    let module_url = repo.module_url(coord);
    match downloader.fetch_text_internal(&module_url).await {
        Ok(content) => {
            downloader.record_connection_success(&repo.url).await;
            if let Ok(meta) = GradleModuleMetadata::parse(&content) {
                let _ = cache.store_file(coord, &coord.module_file_name(), content.as_bytes());
                let deps = meta.resolved_dependencies();
                cache.insert_resolved(coord.clone(), deps.clone());
                return Some(deps);
            }
        }
        Err(err) => {
            let is_conn_err = matches!(err, crate::downloader::FetchError::Connection(_));
            downloader.record_probe_error(&repo.url, &err).await;
            if is_conn_err {
                return None;
            }
        }
    }

    let pom_url = repo.pom_url(coord);
    match downloader.fetch_text_internal(&pom_url).await {
        Ok(content) => {
            downloader.record_connection_success(&repo.url).await;
            if let Ok(mut pom) = PomModel::parse(&content) {
                let _ = cache.store_file(coord, &coord.pom_file_name(), content.as_bytes());
                resolve_and_inherit_parent_poms(downloader, repo, registry, &mut pom, cache).await;
                pom.interpolate(&ahash::AHashMap::default());
                let deps = pom.direct_dependencies();
                cache.insert_resolved(coord.clone(), deps.clone());
                return Some(deps);
            }
        }
        Err(err) => downloader.record_probe_error(&repo.url, &err).await,
    }

    None
}

fn evaluate_earliest_resolved(
    results: &[Option<Option<Vec<DependencyCoordinate>>>],
) -> Option<Option<Vec<DependencyCoordinate>>> {
    for outcome in results {
        match outcome {
            Some(Some(deps)) => return Some(Some(deps.clone())),
            Some(None) => continue,
            None => return None,
        }
    }
    Some(None)
}
