use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use ahash::AHashSet;
use smol_str::SmolStr;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::Url;

use crate::gradle::config::GradleConfig;
use crate::gradle::dependency::{DependencyCoordinate, deduplicate_newest};
use crate::gradle::discovery::{
    discover_workspace_pipeline_context, resolve_highest_kotlin_version,
};
use crate::gradle::stages::direct_deps::{DirectDepsResult, run_direct_deps_stage};
use crate::gradle::stages::fast_path::{FastPathResult, run_fast_path_stage};
use crate::gradle::stages::transitive::{TransitiveStageResult, run_transitive_stage};
pub use crate::gradle::summary::IndexingSummary;
use crate::indexer::{FileSymbols, IndexSegment, SegmentKey, WorkspaceIndex};
use crate::progress::ProgressReporter;

pub struct MultiStageIndexer;

impl MultiStageIndexer {
    /// Asynchronous background spawner offloaded to blocking pool to prevent Tokio loop starvation.
    pub fn spawn(
        workspace_root: PathBuf,
        config: GradleConfig,
        index: Arc<RwLock<WorkspaceIndex>>,
    ) -> tokio::task::JoinHandle<IndexingSummary> {
        Self::spawn_with_progress(workspace_root, config, index, ProgressReporter::noop())
    }

    /// Asynchronous background spawner with progress reporting.
    pub fn spawn_with_progress(
        workspace_root: PathBuf,
        config: GradleConfig,
        index: Arc<RwLock<WorkspaceIndex>>,
        progress: ProgressReporter,
    ) -> tokio::task::JoinHandle<IndexingSummary> {
        tokio::spawn(async move {
            run_progressive_pipeline(workspace_root, config, index, progress).await
        })
    }
}

async fn run_progressive_pipeline(
    workspace_root: PathBuf,
    config: GradleConfig,
    index: Arc<RwLock<WorkspaceIndex>>,
    mut progress: ProgressReporter,
) -> IndexingSummary {
    let start = Instant::now();
    let mut summary = IndexingSummary::default();
    let mut indexed_keys = AHashSet::default();
    let mut processed_archives = AHashSet::default();

    progress.begin("Indexing Kotlin").await;
    progress.report("Discovering dependencies", 0, 4).await;

    let workspace_path = workspace_root.clone();
    let discovery =
        tokio::task::spawn_blocking(move || discover_workspace_pipeline_context(&workspace_path))
            .await
            .unwrap_or_default();
    summary.catalogs_found = discovery.catalogs.len();
    summary.build_scripts_found = discovery.scripts.len();
    summary.total_dependencies_declared = discovery.declared_dependencies.len();

    let deduped_direct = deduplicate_newest(discovery.declared_dependencies);
    let highest_kotlin = resolve_highest_kotlin_version(&discovery.kotlin_versions);
    let has_gradle_files = !discovery.catalogs.is_empty() || !discovery.scripts.is_empty();

    // Stage 1: Fast Path (Kotlin Stdlib & Android SDK)
    progress
        .report("1/3 Kotlin stdlib & Android SDK", 1, 4)
        .await;
    let fast_path = async_stage_fast_path(
        &workspace_root,
        &config,
        highest_kotlin.as_deref(),
        discovery.preferred_sdk,
        has_gradle_files,
        &mut summary,
        &mut indexed_keys,
        &mut processed_archives,
    )
    .await;
    summary.kotlin_version = Some(fast_path.kotlin_version.clone());
    flush_segment(&index, SegmentKey::fast_path(), fast_path.symbols).await;

    // Stage 2: Direct Dependencies
    progress.report("2/3 Direct dependencies", 2, 4).await;
    let direct_res = async_stage_direct_deps(
        &deduped_direct,
        &config,
        &mut summary,
        &mut indexed_keys,
        &mut processed_archives,
    )
    .await;
    flush_segment(&index, SegmentKey::direct_deps(), direct_res.symbols).await;

    // Stage 3: Transitive Graph & Delta Indexing
    progress.report("3/3 Transitive dependencies", 3, 4).await;
    let trans_res = async_stage_transitive(
        &deduped_direct,
        &config,
        &indexed_keys,
        &mut processed_archives,
        &mut summary,
    )
    .await;
    flush_transitive_stage(&index, trans_res, &config).await;

    summary.duration = start.elapsed();
    summary.print_summary(config.indexing_logging);
    progress.finish("Indexing complete").await;
    summary
}

async fn flush_transitive_stage(
    index: &Arc<RwLock<WorkspaceIndex>>,
    trans_res: TransitiveStageResult,
    config: &GradleConfig,
) {
    let sym_count = trans_res.symbols.len();
    let flush_start = Instant::now();
    flush_segment(index, SegmentKey::transitive(), trans_res.symbols).await;
    if config.indexing_logging.is_enabled() && sym_count > 0 {
        crate::logger::log!(
            "indexing",
            "Stage 3/3: Ingested {} archive entries into index in {:?}",
            sym_count,
            flush_start.elapsed()
        );
    }
}

pub async fn flush_segment(
    index: &Arc<RwLock<WorkspaceIndex>>,
    key: SegmentKey,
    symbols: Vec<(Url, FileSymbols)>,
) {
    if symbols.is_empty() {
        return;
    }
    let segment =
        tokio::task::spawn_blocking(move || IndexSegment::from_symbols_parallel(key, symbols))
            .await
            .unwrap_or_else(|_| IndexSegment::new(SegmentKey::from("batch")));

    let mut shared = index.write().await;
    shared.add_segment(segment);
}

pub async fn flush_symbols(index: &Arc<RwLock<WorkspaceIndex>>, symbols: Vec<(Url, FileSymbols)>) {
    flush_segment(index, SegmentKey::from("batch"), symbols).await;
}

async fn async_stage_fast_path(
    workspace_root: &Path,
    config: &GradleConfig,
    kotlin_version: Option<&str>,
    pref_sdk: Option<u32>,
    has_gradle_files: bool,
    summary: &mut IndexingSummary,
    indexed_keys: &mut AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &mut AHashSet<PathBuf>,
) -> FastPathResult {
    let ws = workspace_root.to_path_buf();
    let cfg = config.clone();
    let kv = kotlin_version.map(|s| s.to_string());
    let fast_path = tokio::task::spawn_blocking(move || {
        run_fast_path_stage(&ws, &cfg, kv.as_deref(), pref_sdk, has_gradle_files)
    })
    .await
    .unwrap_or_else(|_| FastPathResult {
        android_platform: None,
        stdlib_artifact: None,
        symbols: vec![],
        metrics: vec![],
        kotlin_version: "".to_owned(),
    });

    apply_fast_path_summary(
        &fast_path,
        summary,
        indexed_keys,
        processed_archives,
        config.prefer_sources,
    );
    fast_path
}

async fn async_stage_direct_deps(
    direct_deps: &[DependencyCoordinate],
    config: &GradleConfig,
    summary: &mut IndexingSummary,
    indexed_keys: &mut AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &mut AHashSet<PathBuf>,
) -> DirectDepsResult {
    let dd = direct_deps.to_vec();
    let cfg = config.clone();
    let keys = indexed_keys.clone();
    let archives = processed_archives.clone();
    let direct_res =
        tokio::task::spawn_blocking(move || run_direct_deps_stage(&dd, &cfg, &keys, &archives))
            .await
            .unwrap_or_else(|_| DirectDepsResult {
                resolved_artifacts: vec![],
                symbols: vec![],
                metrics: vec![],
            });

    apply_direct_deps_summary(
        &direct_res,
        summary,
        indexed_keys,
        processed_archives,
        config.prefer_sources,
    );
    direct_res
}

async fn async_stage_transitive(
    direct_deps: &[DependencyCoordinate],
    config: &GradleConfig,
    indexed_keys: &AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &mut AHashSet<PathBuf>,
    summary: &mut IndexingSummary,
) -> TransitiveStageResult {
    let dd = direct_deps.to_vec();
    let cfg = config.clone();
    let keys = indexed_keys.clone();
    let archives = processed_archives.clone();
    let trans_res =
        tokio::task::spawn_blocking(move || run_transitive_stage(&dd, &cfg, &keys, &archives))
            .await
            .unwrap_or_else(|_| TransitiveStageResult {
                graph_result: crate::gradle::graph::TransitiveGraphResult::default(),
                delta_artifacts: vec![],
                symbols: vec![],
                metrics: vec![],
            });

    apply_transitive_summary(
        &trans_res,
        summary,
        processed_archives,
        config.prefer_sources,
    );
    trans_res
}

fn apply_fast_path_summary(
    fast_path: &FastPathResult,
    summary: &mut IndexingSummary,
    indexed_keys: &mut AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &mut AHashSet<PathBuf>,
    prefer_sources: bool,
) {
    if let Some(ref p) = fast_path.android_platform {
        summary.android_platform = Some(p.platform_name.clone());
        summary.android_sdk_jar = Some(p.android_jar.clone());
        processed_archives.insert(crate::gradle::cache_resolver::normalize_archive_path(
            &p.android_jar,
        ));
    }
    if let Some(ref stdlib) = fast_path.stdlib_artifact {
        if summary.kotlin_version.is_none() {
            summary.kotlin_version = stdlib
                .coordinate
                .version
                .as_ref()
                .map(|v| v.raw().to_string());
        }
        indexed_keys.insert((
            stdlib.coordinate.group.clone(),
            stdlib.coordinate.name.clone(),
        ));
        stdlib.record_processed(processed_archives, prefer_sources);
        summary.resolved_artifacts.push(stdlib.clone());
        summary.direct_dependencies_resolved += 1;
    }
    let sym_count: usize = fast_path.metrics.iter().map(|m| m.symbol_count).sum();
    summary.total_symbols_indexed += sym_count;
    summary.archive_metrics.extend(fast_path.metrics.clone());
    summary.unique_dependencies_indexed = summary.resolved_artifacts.len();
}

fn apply_direct_deps_summary(
    direct_res: &DirectDepsResult,
    summary: &mut IndexingSummary,
    indexed_keys: &mut AHashSet<(SmolStr, SmolStr)>,
    processed_archives: &mut AHashSet<PathBuf>,
    prefer_sources: bool,
) {
    for artifact in &direct_res.resolved_artifacts {
        indexed_keys.insert((
            artifact.coordinate.group.clone(),
            artifact.coordinate.name.clone(),
        ));
        artifact.record_processed(processed_archives, prefer_sources);
        summary.resolved_artifacts.push(artifact.clone());
    }
    summary.direct_dependencies_resolved += direct_res.resolved_artifacts.len();
    let sym_count: usize = direct_res.metrics.iter().map(|m| m.symbol_count).sum();
    summary.total_symbols_indexed += sym_count;
    summary.archive_metrics.extend(direct_res.metrics.clone());
    summary.unique_dependencies_indexed = summary.resolved_artifacts.len();
}

fn apply_transitive_summary(
    trans_res: &TransitiveStageResult,
    summary: &mut IndexingSummary,
    processed_archives: &mut AHashSet<PathBuf>,
    prefer_sources: bool,
) {
    summary.graph_dependencies_resolved = trans_res.graph_result.resolved_dependencies.len();
    summary.transitive_dependencies_resolved = trans_res.graph_result.transitive_count;
    summary.descriptors_parsed = trans_res.graph_result.descriptors_parsed;

    for artifact in &trans_res.delta_artifacts {
        artifact.record_processed(processed_archives, prefer_sources);
        summary.resolved_artifacts.push(artifact.clone());
    }
    summary.transitive_dependencies_indexed += trans_res.delta_artifacts.len();
    let sym_count: usize = trans_res.metrics.iter().map(|m| m.symbol_count).sum();
    summary.total_symbols_indexed += sym_count;
    summary.archive_metrics.extend(trans_res.metrics.clone());
    summary.unique_dependencies_indexed = summary.resolved_artifacts.len();
}
