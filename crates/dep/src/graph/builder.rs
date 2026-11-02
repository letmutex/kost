use ahash::{AHashMap, AHashSet};
use smol_str::SmolStr;
use std::collections::VecDeque;

use super::{GraphNode, SolvedGraph};
use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;

pub(crate) fn drain_unvisited_batch(
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

pub(crate) fn process_resolved_transitives(
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

pub(crate) fn build_solved_graph(
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
            for child_key in child_keys {
                if let Some(&child_idx) = key_to_idx.get(child_key)
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
