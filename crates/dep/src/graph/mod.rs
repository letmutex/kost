use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;
use std::collections::VecDeque;
use std::path::PathBuf;

use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::repo::RepositoryRegistry;

mod builder;
mod task;

use builder::{build_solved_graph, drain_unvisited_batch};
use task::{GraphBuildState, process_batch_join_set, spawn_resolution_tasks};

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
