use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::module::GradleModuleMetadata;
use crate::pom::PomModel;
use crate::repo::{Repository, RepositoryRegistry};

/// Resolve metadata for a dependency coordinate across repositories.
pub async fn resolve_metadata(
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    coord: &DependencyCoordinate,
    cache: &GradleCache,
) -> Result<Vec<DependencyCoordinate>, String> {
    if coord.version.is_none() {
        return Ok(Vec::new());
    }

    if let Some(cached_deps) = try_resolve_cached_transitives(coord, cache) {
        return Ok(cached_deps);
    }

    let candidates = coord.kmp_candidate_names();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let mut primary_target = coord.clone();
    primary_target.name = smol_str::SmolStr::new(&candidates[0]);
    if let Some(deps) = probe_remote_metadata(downloader, registry, &primary_target, cache).await {
        return Ok(deps);
    }

    if let Some(deps) =
        probe_candidate_variants(downloader, registry, coord, &candidates, cache).await
    {
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

fn resolve_parent_pom_inheritance(pom: &mut PomModel, cache: &GradleCache) {
    resolve_parent_pom_depth(pom, cache, 0);
}

fn resolve_parent_pom_depth(pom: &mut PomModel, cache: &GradleCache, depth: usize) {
    if depth >= 4 {
        return;
    }
    if let Some(ref parent_info) = pom.parent
        && let parent_coord = DependencyCoordinate::new(
            parent_info.group.clone(),
            parent_info.name.clone(),
            Some(&parent_info.version),
        )
        && let Some(parent_path) = cache.find_pom(&parent_coord)
        && let Ok(content) = std::fs::read_to_string(&parent_path)
        && let Ok(mut parent_pom) = PomModel::parse(&content)
    {
        resolve_parent_pom_depth(&mut parent_pom, cache, depth + 1);
        parent_pom.interpolate(&ahash::AHashMap::default());
        pom.inherit_from_parent(&parent_pom);
    }
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
        return probe_repo_metadata(downloader, &repos[0], coord, cache).await;
    }

    let mut join_set = tokio::task::JoinSet::new();
    for (idx, repo) in repos.iter().enumerate() {
        let downloader_clone = downloader.clone();
        let repo_clone = repo.clone();
        let coord_clone = coord.clone();
        let cache_ref = cache.clone();
        join_set.spawn(async move {
            let res =
                probe_repo_metadata(&downloader_clone, &repo_clone, &coord_clone, &cache_ref).await;
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
                return Some(meta.resolved_dependencies());
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
                resolve_parent_pom_inheritance(&mut pom, cache);
                pom.interpolate(&ahash::AHashMap::default());
                return Some(pom.direct_dependencies());
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

fn try_resolve_cached_transitives(
    coord: &DependencyCoordinate,
    cache: &GradleCache,
) -> Option<Vec<DependencyCoordinate>> {
    if coord.version.is_none() {
        return Some(Vec::new());
    }

    if let Some(module_path) = cache.find_module(coord)
        && let Ok(content) = std::fs::read_to_string(module_path)
        && let Ok(meta) = GradleModuleMetadata::parse(&content)
    {
        return Some(meta.resolved_dependencies());
    }

    if let Some(pom_path) = cache.find_pom(coord)
        && let Ok(content) = std::fs::read_to_string(pom_path)
        && let Ok(mut pom) = PomModel::parse(&content)
    {
        resolve_parent_pom_inheritance(&mut pom, cache);
        pom.interpolate(&ahash::AHashMap::default());
        return Some(pom.direct_dependencies());
    }

    None
}

/// Resolves transitive dependencies from locally cached Gradle module metadata or POM file.
pub fn resolve_offline_transitives(
    coord: &DependencyCoordinate,
    cache: &GradleCache,
) -> Vec<DependencyCoordinate> {
    try_resolve_cached_transitives(coord, cache).unwrap_or_default()
}
