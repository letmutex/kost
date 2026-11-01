use std::collections::VecDeque;
use std::path::Path;

use ahash::{AHashMap, AHashSet};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smol_str::SmolStr;

use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::descriptor::ModuleDescriptor;
use crate::module::GradleModuleMetadata;
use crate::pom::PomModel;

/// Try to resolve direct transitive dependencies of `coord` from cached `.module` or `.pom` files.
pub fn try_resolve_cached_transitives(
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
        pom.resolve_parent_from_cache(cache);
        pom.interpolate(&AHashMap::default());
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransitiveGraphResult {
    pub resolved_dependencies: Vec<DependencyCoordinate>,
    pub direct_count: usize,
    pub transitive_count: usize,
    pub descriptors_parsed: usize,
}

pub struct DependencyGraph;

impl DependencyGraph {
    pub fn resolve_transitive_graph(
        direct_deps: &[DependencyCoordinate],
        gradle_user_home: Option<&Path>,
    ) -> TransitiveGraphResult {
        let mut final_deps: AHashMap<(SmolStr, SmolStr), DependencyCoordinate> =
            AHashMap::default();
        let mut queue: VecDeque<DependencyCoordinate> = VecDeque::new();

        for dep in crate::coordinate::deduplicate_newest(direct_deps.iter().cloned()) {
            final_deps.insert(dep.kmp_key(), dep.clone());
            queue.push_back(dep);
        }

        let direct_count = final_deps.len();
        let descriptors_parsed = if let Some(home) = gradle_user_home {
            expand_cached_graph_queue(home, &mut final_deps, &mut queue)
        } else {
            0
        };

        let mut resolved: Vec<DependencyCoordinate> = final_deps.into_values().collect();
        resolved.sort_by(|a, b| (&a.group, &a.name).cmp(&(&b.group, &b.name)));

        let total = resolved.len();
        let transitive_count = total.saturating_sub(direct_count);

        TransitiveGraphResult {
            resolved_dependencies: resolved,
            direct_count,
            transitive_count,
            descriptors_parsed,
        }
    }
}

fn expand_cached_graph_queue(
    home: &Path,
    final_deps: &mut AHashMap<(SmolStr, SmolStr), DependencyCoordinate>,
    queue: &mut VecDeque<DependencyCoordinate>,
) -> usize {
    let metadata_dirs = ModuleDescriptor::discover_metadata_dirs(home);
    let gradle_cache = GradleCache::new(Some(home));
    let mut visited: AHashSet<(SmolStr, SmolStr, SmolStr)> = AHashSet::default();
    let mut descriptors_parsed = 0;

    while !queue.is_empty() {
        let to_fetch: Vec<DependencyCoordinate> = queue
            .drain(..)
            .filter(|dep| is_active_unvisited_dep(dep, final_deps, &mut visited))
            .collect();

        if to_fetch.is_empty() {
            continue;
        }

        let resolved_batches: Vec<(bool, Vec<DependencyCoordinate>)> = to_fetch
            .into_par_iter()
            .filter_map(|dep| {
                if let Some(desc) = ModuleDescriptor::find_in_cache_with_dirs(&metadata_dirs, &dep)
                {
                    return Some((true, desc.dependencies));
                }
                let cache = gradle_cache.as_ref()?;
                let transitives = try_resolve_cached_transitives(&dep, cache)?;
                Some((false, transitives))
            })
            .collect();

        for (from_descriptor, dependencies) in resolved_batches {
            if from_descriptor {
                descriptors_parsed += 1;
            }
            for trans_dep in dependencies {
                process_transitive_dep(trans_dep, final_deps, queue);
            }
        }
    }

    descriptors_parsed
}

fn is_active_unvisited_dep(
    dep: &DependencyCoordinate,
    final_deps: &AHashMap<(SmolStr, SmolStr), DependencyCoordinate>,
    visited: &mut AHashSet<(SmolStr, SmolStr, SmolStr)>,
) -> bool {
    let kmp_key = dep.kmp_key();
    if let Some(active) = final_deps.get(&kmp_key)
        && active.version != dep.version
    {
        return false;
    }
    let ver_str = dep
        .version
        .as_ref()
        .map(|ver| SmolStr::new(ver.raw()))
        .unwrap_or_default();
    visited.insert((dep.group.clone(), dep.name.clone(), ver_str))
}

fn process_transitive_dep(
    trans_dep: DependencyCoordinate,
    final_deps: &mut AHashMap<(SmolStr, SmolStr), DependencyCoordinate>,
    queue: &mut VecDeque<DependencyCoordinate>,
) {
    let key = trans_dep.kmp_key();
    if let Some(existing) = final_deps.get_mut(&key) {
        let should_promote = match (&trans_dep.version, &existing.version) {
            (Some(new_ver), Some(old_ver)) => match new_ver.cmp(old_ver) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => trans_dep.kmp_target() > existing.kmp_target(),
                std::cmp::Ordering::Less => false,
            },
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => trans_dep.kmp_target() > existing.kmp_target(),
        };
        if should_promote {
            *existing = trans_dep.clone();
            queue.push_back(trans_dep);
        }
    } else {
        final_deps.insert(key, trans_dep.clone());
        queue.push_back(trans_dep);
    }
}
