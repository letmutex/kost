use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::pom::PomModel;
use crate::repo::{Repository, RepositoryRegistry};

pub(crate) async fn resolve_and_inherit_parent_poms(
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
        let parent_pom = if let Some(cached) = cache.get_or_parse_pom(&parent_coord) {
            Some(cached)
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

pub(crate) async fn fetch_and_cache_parent_pom(
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
            cache.insert_parent_pom(parent_coord.clone(), parent_pom.clone());
            return Some(parent_pom);
        }
    }
    None
}
