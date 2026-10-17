use std::path::PathBuf;
use std::time::Instant;

use ahash::AHashSet;
use smol_str::SmolStr;
use tower_lsp::lsp_types::Url;

use super::{log_resolved_artifact, prepare_archive_paths};
use crate::gradle::archive::{ArchiveIndexer, ArchiveProcessingMetric};
use crate::gradle::cache_resolver::{GradleCacheResolver, ResolvedArtifact};
use crate::gradle::config::GradleConfig;
use crate::gradle::dependency::DependencyCoordinate;
use crate::indexer::FileSymbols;

pub struct DirectDepsResult {
    pub resolved_artifacts: Vec<ResolvedArtifact>,
    pub symbols: Vec<(Url, FileSymbols)>,
    pub metrics: Vec<ArchiveProcessingMetric>,
}

pub fn run_direct_deps_stage(
    direct_deps: &[DependencyCoordinate],
    config: &GradleConfig,
    already_indexed_coords: &AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &AHashSet<PathBuf>,
) -> DirectDepsResult {
    let start = Instant::now();
    let filtered_deps: Vec<_> = direct_deps
        .iter()
        .filter(|dep| !already_indexed_coords.contains(&(dep.group.clone(), dep.name.clone())))
        .cloned()
        .collect();

    if config.indexing_logging.is_enabled() {
        crate::logger::log!(
            "indexing",
            "Stage 2/3: Resolving and indexing {} direct dependencies...",
            filtered_deps.len()
        );
    }

    let resolved_artifacts = GradleCacheResolver::resolve_all(&filtered_deps, config);
    let mut unique_artifacts = Vec::new();
    let mut local_processed = processed_archives.clone();

    for artifact in resolved_artifacts {
        if !artifact.is_already_processed(&local_processed, config.prefer_sources) {
            artifact.record_processed(&mut local_processed, config.prefer_sources);
            unique_artifacts.push(artifact);
        }
    }

    if config.indexing_logging.is_verbose() {
        for artifact in &unique_artifacts {
            log_resolved_artifact(artifact, config.prefer_sources);
        }
    }

    let archives = prepare_archive_paths(&unique_artifacts, None, config.prefer_sources);
    let (symbols, metrics) =
        ArchiveIndexer::index_archives_parallel(&archives, config.prefer_sources);
    let total_syms: usize = metrics.iter().map(|m| m.symbol_count).sum();

    if config.indexing_logging.is_enabled() {
        crate::logger::log!(
            "indexing",
            "Stage 2/3 done in {:?}: {} symbols from {} archives ({} direct libraries resolved in cache out of {})",
            start.elapsed(),
            total_syms,
            archives.len(),
            unique_artifacts.len(),
            filtered_deps.len()
        );
    }

    DirectDepsResult {
        resolved_artifacts: unique_artifacts,
        symbols,
        metrics,
    }
}
