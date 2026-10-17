use std::collections::VecDeque;
use std::path::Path;

use ahash::{AHashMap, AHashSet};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use smol_str::SmolStr;

use crate::gradle::dependency::DependencyCoordinate;
use crate::gradle::descriptor::ModuleDescriptor;

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
        let mut visited_descriptors: AHashSet<(SmolStr, SmolStr, SmolStr)> = AHashSet::default();
        let mut descriptors_parsed = 0;

        for dep in direct_deps {
            final_deps.insert((dep.group.clone(), dep.name.clone()), dep.clone());
            queue.push_back(dep.clone());
        }

        let direct_count = final_deps.len();

        if let Some(home) = gradle_user_home {
            let metadata_dirs = ModuleDescriptor::discover_metadata_dirs(home);
            if !metadata_dirs.is_empty() {
                while !queue.is_empty() {
                    let current_batch: Vec<DependencyCoordinate> = queue.drain(..).collect();
                    let to_fetch: Vec<DependencyCoordinate> = current_batch
                        .into_iter()
                        .filter(|dep| {
                            let key = (dep.group.clone(), dep.name.clone());
                            if let Some(active) = final_deps.get(&key)
                                && active.version != dep.version
                            {
                                return false;
                            }
                            let ver_str = dep
                                .version
                                .as_ref()
                                .map(|v| SmolStr::new(v.raw()))
                                .unwrap_or_default();
                            let desc_key = (key.0, key.1, ver_str);
                            visited_descriptors.insert(desc_key)
                        })
                        .collect();

                    if to_fetch.is_empty() {
                        continue;
                    }

                    let parsed_descriptors: Vec<ModuleDescriptor> = to_fetch
                        .into_par_iter()
                        .filter_map(|dep| {
                            ModuleDescriptor::find_in_cache_with_dirs(&metadata_dirs, &dep)
                        })
                        .collect();

                    for desc in parsed_descriptors {
                        descriptors_parsed += 1;
                        for trans_dep in desc.dependencies {
                            process_transitive_dep(trans_dep, &mut final_deps, &mut queue);
                        }
                    }
                }
            }
        }

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

fn process_transitive_dep(
    trans_dep: DependencyCoordinate,
    final_deps: &mut AHashMap<(SmolStr, SmolStr), DependencyCoordinate>,
    queue: &mut VecDeque<DependencyCoordinate>,
) {
    let key = (trans_dep.group.clone(), trans_dep.name.clone());
    if let Some(existing) = final_deps.get_mut(&key) {
        if let (Some(new_ver), Some(old_ver)) = (&trans_dep.version, &existing.version)
            && new_ver.cmp(old_ver).is_gt()
        {
            *existing = trans_dep.clone();
            queue.push_back(trans_dep);
        }
    } else {
        final_deps.insert(key, trans_dep.clone());
        queue.push_back(trans_dep);
    }
}
