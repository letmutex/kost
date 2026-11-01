use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::module::GradleModuleMetadata;
use crate::pom::PomModel;
use crate::repo::{Repository, RepositoryRegistry};

pub use resolver::graph::resolve_offline_transitives;
use resolver::graph::try_resolve_cached_transitives;

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

    let candidates = coord.kmp_probe_candidates();
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
        let cache_ref = cache.clone();
        join_set.spawn(async move {
            let res = probe_repo_metadata(
                &downloader_clone,
                &repo_clone,
                &registry_clone,
                &coord_clone,
                &cache_ref,
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
                resolve_and_inherit_parent_poms(downloader, repo, registry, &mut pom, cache).await;
                pom.interpolate(&ahash::AHashMap::default());
                return Some(pom.direct_dependencies());
            }
        }
        Err(err) => downloader.record_probe_error(&repo.url, &err).await,
    }

    None
}

async fn resolve_and_inherit_parent_poms(
    downloader: &Downloader,
    primary_repo: &Repository,
    registry: &RepositoryRegistry,
    pom: &mut PomModel,
    cache: &GradleCache,
) {
    let mut ancestors = Vec::new();
    let mut next_parent = pom.parent.clone();
    for _ in 0..4 {
        let Some(parent_info) = next_parent else {
            break;
        };
        let parent_coord = DependencyCoordinate::new(
            parent_info.group,
            parent_info.name,
            Some(&parent_info.version),
        );
        let parent_pom = if let Some(cached_path) = cache.find_pom(&parent_coord) {
            std::fs::read_to_string(cached_path)
                .ok()
                .and_then(|xml| PomModel::parse(&xml).ok())
        } else {
            fetch_and_cache_parent_pom(downloader, primary_repo, registry, &parent_coord, cache)
                .await
        };
        let Some(parsed_parent) = parent_pom else {
            break;
        };
        next_parent = parsed_parent.parent.clone();
        ancestors.push(parsed_parent);
    }
    for parent_pom in ancestors {
        pom.inherit_from_parent(&parent_pom);
    }
}

async fn fetch_and_cache_parent_pom(
    downloader: &Downloader,
    primary_repo: &Repository,
    registry: &RepositoryRegistry,
    parent_coord: &DependencyCoordinate,
    cache: &GradleCache,
) -> Option<PomModel> {
    let candidate_repos = std::iter::once(primary_repo).chain(
        registry
            .repositories
            .iter()
            .filter(|repo| repo.url != primary_repo.url),
    );
    for repo in candidate_repos {
        if downloader.is_repo_unreachable(&repo.url).await {
            continue;
        }
        let parent_url = repo.pom_url(parent_coord);
        if let Ok(content) = downloader.fetch_text_internal(&parent_url).await
            && let Ok(parent_pom) = PomModel::parse(&content)
        {
            downloader.record_connection_success(&repo.url).await;
            let _ = cache.store_file(
                parent_coord,
                &parent_coord.pom_file_name(),
                content.as_bytes(),
            );
            return Some(parent_pom);
        }
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
