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
use crate::gradle::graph::{DependencyGraph, TransitiveGraphResult};
use crate::indexer::FileSymbols;

pub struct TransitiveStageResult {
    pub graph_result: TransitiveGraphResult,
    pub delta_artifacts: Vec<ResolvedArtifact>,
    pub symbols: Vec<(Url, FileSymbols)>,
    pub metrics: Vec<ArchiveProcessingMetric>,
}

pub fn run_transitive_stage(
    direct_deps: &[DependencyCoordinate],
    config: &GradleConfig,
    indexed_keys: &AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &AHashSet<PathBuf>,
) -> TransitiveStageResult {
    let start = Instant::now();
    let gradle_home = config.resolve_gradle_user_home();
    if config.indexing_logging.is_enabled() {
        crate::logger::log!(
            "indexing",
            "Stage 3/3: Resolving transitive dependency graph..."
        );
    }
    let graph_start = Instant::now();
    let graph_result =
        DependencyGraph::resolve_transitive_graph(direct_deps, gradle_home.as_deref());

    let delta_deps: Vec<DependencyCoordinate> = graph_result
        .resolved_dependencies
        .iter()
        .filter(|dep| !indexed_keys.contains(&(dep.group.clone(), dep.name.clone())))
        .cloned()
        .collect();

    if config.indexing_logging.is_enabled() {
        crate::logger::log!(
            "indexing",
            "Stage 3/3: Graph resolved in {:?} ({} dependencies, parsed {} descriptors); resolving and indexing {} delta dependencies...",
            graph_start.elapsed(),
            graph_result.resolved_dependencies.len(),
            graph_result.descriptors_parsed,
            delta_deps.len()
        );
    }

    let delta_artifacts = GradleCacheResolver::resolve_all(&delta_deps, config);
    let mut unique_artifacts = Vec::new();
    let mut local_processed = processed_archives.clone();

    for artifact in delta_artifacts {
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
            "Stage 3/3 done in {:?}: {} symbols from {} archives ({} delta libraries resolved in cache out of {})",
            start.elapsed(),
            total_syms,
            archives.len(),
            unique_artifacts.len(),
            delta_deps.len()
        );
    }

    TransitiveStageResult {
        graph_result,
        delta_artifacts: unique_artifacts,
        symbols,
        metrics,
    }
}
