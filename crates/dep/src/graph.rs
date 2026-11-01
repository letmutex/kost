use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;
use std::collections::VecDeque;
use std::path::PathBuf;

use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::metadata::resolve_offline_transitives;
use crate::repo::RepositoryRegistry;

/// A node in the solved dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    pub coordinate: DependencyCoordinate,
    pub is_direct: bool,
    pub promoted_from: Option<String>,
    pub children: Vec<usize>,
    pub cached_binary: Option<PathBuf>,
    pub cached_sources: Option<PathBuf>,
}

/// The complete solved dependency graph result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SolvedGraph {
    pub nodes: Vec<GraphNode>,
    pub direct_count: usize,
    pub transitive_count: usize,
}

impl SolvedGraph {
    pub async fn solve(
        direct_deps: &[DependencyCoordinate],
        registry: &RepositoryRegistry,
        cache: &GradleCache,
        downloader: &Downloader,
        offline: bool,
    ) -> Self {
        Self::solve_with_progress(direct_deps, registry, cache, downloader, offline, None).await
    }

    pub async fn solve_with_progress(
        direct_deps: &[DependencyCoordinate],
        registry: &RepositoryRegistry,
        cache: &GradleCache,
        downloader: &Downloader,
        offline: bool,
        mut progress: Option<&mut crate::progress::ResolveProgress>,
    ) -> Self {
        let deduped_direct = crate::coordinate::deduplicate_newest(direct_deps.iter().cloned());
        let mut final_deps: AHashMap<
            (SmolStr, SmolStr),
            (DependencyCoordinate, bool, Option<String>),
        > = AHashMap::default();
        let mut queue: VecDeque<DependencyCoordinate> = VecDeque::new();
        let mut visited: AHashSet<(SmolStr, SmolStr, SmolStr)> = AHashSet::default();
        let mut parent_to_children: AHashMap<(SmolStr, SmolStr), Vec<(SmolStr, SmolStr)>> =
            AHashMap::default();

        for dep in deduped_direct {
            final_deps.insert(dep.kmp_key(), (dep.clone(), true, None));
            queue.push_back(dep);
        }

        let direct_count = final_deps.len();
        let mut resolved_count = 0;
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        while !queue.is_empty() {
            let batch = drain_unvisited_batch(&mut queue, &mut visited, &final_deps);
            if batch.is_empty() {
                break;
            }

            let in_flight: VecDeque<DependencyCoordinate> = batch.clone().into();
            if let Some(ref mut active_progress) = progress
                && let Some(first_in_flight) = in_flight.front()
            {
                let total_count =
                    (resolved_count + in_flight.len() + queue.len()).max(resolved_count);
                active_progress.on_update(resolved_count, total_count, first_in_flight);
            }

            let join_set = spawn_resolution_tasks(batch, downloader, registry, cache, offline);
            let mut state = GraphBuildState {
                final_deps: &mut final_deps,
                parent_to_children: &mut parent_to_children,
                queue: &mut queue,
            };
            process_batch_join_set(
                join_set,
                in_flight,
                &mut ticker,
                &mut state,
                &mut resolved_count,
                progress.as_deref_mut(),
            )
            .await;
        }

        if let Some(ref mut active_progress) = progress {
            active_progress.finish();
        }

        build_solved_graph(final_deps, parent_to_children, direct_count, cache)
    }
}

struct GraphBuildState<'a> {
    final_deps: &'a mut AHashMap<(SmolStr, SmolStr), (DependencyCoordinate, bool, Option<String>)>,
    parent_to_children: &'a mut AHashMap<(SmolStr, SmolStr), Vec<(SmolStr, SmolStr)>>,
    queue: &'a mut VecDeque<DependencyCoordinate>,
}

async fn process_batch_join_set(
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

fn drain_unvisited_batch(
    queue: &mut VecDeque<DependencyCoordinate>,
    visited: &mut AHashSet<(SmolStr, SmolStr, SmolStr)>,
    final_deps: &AHashMap<(SmolStr, SmolStr), (DependencyCoordinate, bool, Option<String>)>,
) -> Vec<DependencyCoordinate> {
    let mut batch = Vec::new();
    while let Some(current) = queue.pop_front() {
        let key = current.kmp_key();
        if let Some((active, _, _)) = final_deps.get(&key)
            && active.version != current.version
        {
            continue;
        }
        let ver_str = current
            .version
            .as_ref()
            .map(|ver| ver.raw.clone())
            .unwrap_or_default();
        let visit_key = (current.group.clone(), current.name.clone(), ver_str);

        if visited.insert(visit_key) {
            batch.push(current);
        }
    }
    batch
}

fn spawn_resolution_tasks(
    batch: Vec<DependencyCoordinate>,
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    cache: &GradleCache,
    offline: bool,
) -> tokio::task::JoinSet<(DependencyCoordinate, Vec<DependencyCoordinate>)> {
    let mut join_set = tokio::task::JoinSet::new();
    for item in batch {
        let dl = downloader.clone();
        let reg = registry.clone();
        let c = cache.clone();
        join_set.spawn(async move {
            let transitives = if offline {
                resolve_offline_transitives(&item, &c)
            } else {
                dl.resolve_metadata(&reg, &item, &c)
                    .await
                    .unwrap_or_default()
            };
            (item, transitives)
        });
    }
    join_set
}

fn process_resolved_transitives(
    current: &DependencyCoordinate,
    transitives: Vec<DependencyCoordinate>,
    final_deps: &mut AHashMap<(SmolStr, SmolStr), (DependencyCoordinate, bool, Option<String>)>,
    parent_to_children: &mut AHashMap<(SmolStr, SmolStr), Vec<(SmolStr, SmolStr)>>,
    queue: &mut VecDeque<DependencyCoordinate>,
) -> usize {
    let parent_key = current.kmp_key();
    let child_entries = parent_to_children.entry(parent_key).or_default();
    let mut newly_queued = 0;

    for trans in transitives {
        let key = trans.kmp_key();
        if !child_entries.contains(&key) {
            child_entries.push(key.clone());
        }

        if let Some((existing_dep, _, promoted)) = final_deps.get_mut(&key) {
            if should_promote_dependency(existing_dep, &trans, promoted) {
                *existing_dep = trans.clone();
                queue.push_back(trans);
                newly_queued += 1;
            }
        } else {
            final_deps.insert(key, (trans.clone(), false, None));
            queue.push_back(trans);
            newly_queued += 1;
        }
    }

    newly_queued
}

fn should_promote_dependency(
    existing: &DependencyCoordinate,
    trans: &DependencyCoordinate,
    promoted: &mut Option<String>,
) -> bool {
    match (&existing.version, &trans.version) {
        (Some(old_ver), Some(new_ver)) => match new_ver.cmp(old_ver) {
            std::cmp::Ordering::Greater => {
                *promoted = Some(old_ver.raw().to_string());
                true
            }
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => trans.kmp_target() > existing.kmp_target(),
        },
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (None, None) => trans.kmp_target() > existing.kmp_target(),
    }
}

fn build_solved_graph(
    final_deps: AHashMap<(SmolStr, SmolStr), (DependencyCoordinate, bool, Option<String>)>,
    parent_to_children: AHashMap<(SmolStr, SmolStr), Vec<(SmolStr, SmolStr)>>,
    direct_count: usize,
    cache: &GradleCache,
) -> SolvedGraph {
    let mut nodes = Vec::new();
    let mut sorted_entries: Vec<_> = final_deps.into_values().collect();
    sorted_entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| (&a.0.group, &a.0.name).cmp(&(&b.0.group, &b.0.name)))
    });

    let mut key_to_idx = ahash::AHashMap::default();
    for (idx, (coord, _, _)) in sorted_entries.iter().enumerate() {
        key_to_idx.insert(coord.kmp_key(), idx);
    }

    for (idx, (coord, is_direct, promoted_from)) in sorted_entries.into_iter().enumerate() {
        let cached_binary = cache.find_artifact(&coord, false);
        let cached_sources = cache.find_artifact(&coord, true);
        let mut children = Vec::new();

        if let Some(child_keys) = parent_to_children.get(&coord.kmp_key()) {
            for ck in child_keys {
                if let Some(&child_idx) = key_to_idx.get(ck)
                    && child_idx != idx
                    && !children.contains(&child_idx)
                {
                    children.push(child_idx);
                }
            }
        }

        nodes.push(GraphNode {
            coordinate: coord,
            is_direct,
            promoted_from,
            children,
            cached_binary,
            cached_sources,
        });
    }

    let total = nodes.len();
    let transitive_count = total.saturating_sub(direct_count);

    SolvedGraph {
        nodes,
        direct_count,
        transitive_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_graph_deduplication_and_conflict_promotion() {
        let cache = GradleCache::new(None).unwrap();
        let downloader = Downloader::new(2);
        let registry = RepositoryRegistry::new();

        let d1 = DependencyCoordinate::parse("androidx.core:core:1.9.0").unwrap();
        let d2 = DependencyCoordinate::parse("androidx.core:core:1.12.0").unwrap();

        let graph = SolvedGraph::solve(&[d1, d2], &registry, &cache, &downloader, true).await;
        assert_eq!(graph.direct_count, 1);
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(
            graph.nodes[0].coordinate.version.as_ref().unwrap().raw(),
            "1.12.0"
        );
    }
}
