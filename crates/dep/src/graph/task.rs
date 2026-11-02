use ahash::AHashMap;
use smol_str::SmolStr;
use std::collections::VecDeque;

use super::builder::process_resolved_transitives;
use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::repo::RepositoryRegistry;

pub(crate) struct GraphBuildState<'a> {
    pub final_deps:
        &'a mut AHashMap<(SmolStr, SmolStr), (DependencyCoordinate, bool, Option<String>)>,
    pub parent_to_children: &'a mut AHashMap<(SmolStr, SmolStr), Vec<(SmolStr, SmolStr)>>,
    pub queue: &'a mut VecDeque<DependencyCoordinate>,
}

pub(crate) fn spawn_resolution_tasks(
    batch: Vec<DependencyCoordinate>,
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    cache: &GradleCache,
    offline: bool,
) -> tokio::task::JoinSet<(DependencyCoordinate, Vec<DependencyCoordinate>)> {
    let mut join_set = tokio::task::JoinSet::new();
    for item in batch {
        let downloader_clone = downloader.clone();
        let registry_clone = registry.clone();
        let cache_clone = cache.clone();
        join_set.spawn(async move {
            let transitives = crate::metadata::resolve_metadata(
                &downloader_clone,
                &registry_clone,
                &item,
                &cache_clone,
                offline,
            )
            .await
            .unwrap_or_default();
            (item, transitives)
        });
    }
    join_set
}

pub(crate) async fn process_batch_join_set(
    mut join_set: tokio::task::JoinSet<(DependencyCoordinate, Vec<DependencyCoordinate>)>,
    mut in_flight: VecDeque<DependencyCoordinate>,
    ticker: &mut tokio::time::Interval,
    state: &mut GraphBuildState<'_>,
    resolved_count: &mut usize,
    mut progress: Option<&mut crate::progress::ResolveProgress>,
) {
    while !join_set.is_empty() {
        tokio::select! {
            res = join_set.join_next() => {
                let Some(res) = res else { break; };
                if let Ok((current, transitives)) = res {
                    if let Some(pos) = in_flight.iter().position(|coord| coord == &current) {
                        in_flight.remove(pos);
                    }
                    process_resolved_transitives(
                        &current,
                        transitives,
                        state.final_deps,
                        state.parent_to_children,
                        state.queue,
                    );
                    *resolved_count += 1;
                    if let Some(ref mut active_progress) = progress {
                        let active_coord = in_flight.front().unwrap_or(&current);
                        let total_count =
                            (*resolved_count + in_flight.len() + state.queue.len()).max(*resolved_count);
                        active_progress.on_update(*resolved_count, total_count, active_coord);
                    }
                }
            }
            _ = ticker.tick() => {
                if let Some(ref mut active_progress) = progress {
                    active_progress.flush_if_dirty();
                }
            }
        }
    }
}
