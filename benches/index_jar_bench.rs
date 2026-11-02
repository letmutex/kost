use lsp::gradle::archive::{ArchiveIndexer, ArchiveProcessingMetric};
use lsp::gradle::cache_resolver::GradleCacheResolver;
use lsp::gradle::catalog::VersionCatalog;
use lsp::gradle::config::GradleConfig;
use lsp::gradle::dependency::deduplicate_newest;
use lsp::gradle::graph::DependencyGraph;
use lsp::indexer::WorkspaceIndex;
use resolver::ResolvedArtifact;
use std::path::Path;
use std::time::{Duration, Instant};

const VERSION_CATALOG_TOML: &str = include_str!("libs.versions.toml");

fn prepare_archive_paths(artifacts: &[ResolvedArtifact], prefer_sources: bool) -> Vec<&Path> {
    let mut archives = Vec::new();
    for artifact in artifacts {
        if prefer_sources && let Some(ref src) = artifact.sources_path {
            archives.push(src.as_path());
            continue;
        }
        archives.push(artifact.binary_path.as_path());
    }
    archives
}

fn print_benchmark_header(config: &GradleConfig) {
    println!("=== Jar & AAR Indexing Benchmark ===");
    println!("Catalog:            benches/libs.versions.toml");
    println!("Prefer sources:     {}", config.prefer_sources);
    if let Some(ref gradle_user_home) = config.resolve_gradle_user_home() {
        println!("Gradle User Home:   {}", gradle_user_home.display());
    }
}

struct ResolutionResults {
    cat_elapsed: Duration,
    graph_elapsed: Duration,
    descriptors_parsed: usize,
    cache_elapsed: Duration,
    resolved_artifacts: Vec<ResolvedArtifact>,
    resolved_count: usize,
}

fn resolve_catalog_artifacts(config: &GradleConfig) -> ResolutionResults {
    println!("\n[1/5] Parsing Version Catalog...");
    let cat_start = Instant::now();
    let catalog = match VersionCatalog::parse(VERSION_CATALOG_TOML) {
        Ok(catalog) => catalog,
        Err(err) => {
            eprintln!("Error: Failed to parse fixed TOML: {err}");
            std::process::exit(1);
        }
    };
    let cat_elapsed = cat_start.elapsed();
    let all_deps = catalog.all_dependencies();
    let deduped_deps = deduplicate_newest(all_deps);
    println!(
        "  -> Parsed {} declared dependencies ({} unique) in {:?}",
        catalog.libraries.len(),
        deduped_deps.len(),
        cat_elapsed
    );

    println!("\n[2/5] Resolving Transitive Dependency Graph...");
    let gradle_home = config.resolve_gradle_user_home();
    let graph_start = Instant::now();
    let graph_result =
        DependencyGraph::resolve_transitive_graph(&deduped_deps, gradle_home.as_deref());
    let graph_elapsed = graph_start.elapsed();
    println!(
        "  -> Resolved {} dependencies ({} direct, {} transitive, parsed {} descriptors) in {:?}",
        graph_result.resolved_dependencies.len(),
        graph_result.direct_count,
        graph_result.transitive_count,
        graph_result.descriptors_parsed,
        graph_elapsed
    );

    println!("\n[3/5] Solving JARs & AARs from local Gradle cache...");
    let cache_start = Instant::now();
    let mut resolved_artifacts =
        GradleCacheResolver::resolve_all(&graph_result.resolved_dependencies, config);
    if let Some(stdlib) = GradleCacheResolver::find_kotlin_stdlib(None, config) {
        resolved_artifacts.push(stdlib);
    }
    let cache_elapsed = cache_start.elapsed();

    let resolved_count = resolved_artifacts.len();
    let total_requested = graph_result.resolved_dependencies.len();
    let sources_count = resolved_artifacts
        .iter()
        .filter(|artifact| artifact.sources_path.is_some())
        .count();

    println!(
        "  -> Resolved {}/{} artifacts ({} with sources.jar) in {:?}",
        resolved_count, total_requested, sources_count, cache_elapsed
    );

    ResolutionResults {
        cat_elapsed,
        graph_elapsed,
        descriptors_parsed: graph_result.descriptors_parsed,
        cache_elapsed,
        resolved_artifacts,
        resolved_count,
    }
}

fn index_and_insert_symbols(
    resolved_artifacts: &[ResolvedArtifact],
    config: &GradleConfig,
) -> (
    WorkspaceIndex,
    Vec<ArchiveProcessingMetric>,
    Duration,
    Duration,
) {
    let archives = prepare_archive_paths(resolved_artifacts, config.prefer_sources);
    println!(
        "\n[4/5] Indexing {} archive(s) in parallel (in-memory bytecode/source parsing)...",
        archives.len()
    );

    let index_start = Instant::now();
    let (symbols, metrics) =
        ArchiveIndexer::index_archives_parallel(&archives, config.prefer_sources);
    let index_elapsed = index_start.elapsed();

    println!(
        "  -> Indexed {} symbols across {} archives in {:?}",
        symbols.len(),
        archives.len(),
        index_elapsed
    );

    println!("\n[5/5] Inserting symbols into WorkspaceIndex...");
    let ins_start = Instant::now();
    let mut workspace_index = WorkspaceIndex::new();
    for (url, file_symbols) in symbols {
        workspace_index.insert_file_symbols(url, file_symbols);
    }
    let ins_elapsed = ins_start.elapsed();

    (workspace_index, metrics, index_elapsed, ins_elapsed)
}

fn print_performance_breakdown(
    resolution: &ResolutionResults,
    index_elapsed: Duration,
    ins_elapsed: Duration,
    total_elapsed: Duration,
    workspace_index: &WorkspaceIndex,
) {
    println!("\n================== Performance Breakdown ==================");
    println!(
        "1. Version Catalog Parsing:      {:>8.2} ms",
        resolution.cat_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "2. Transitive Graph Resolution:  {:>8.2} ms ({} descriptors)",
        resolution.graph_elapsed.as_secs_f64() * 1000.0,
        resolution.descriptors_parsed
    );
    println!(
        "3. Cache Artifact Resolution:    {:>8.2} ms ({} archives)",
        resolution.cache_elapsed.as_secs_f64() * 1000.0,
        resolution.resolved_count
    );
    println!(
        "4. Parallel Archive Indexing:    {:>8.2} ms (Rayon multi-thread)",
        index_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "5. Workspace Index Insertion:    {:>8.2} ms",
        ins_elapsed.as_secs_f64() * 1000.0
    );
    println!("-----------------------------------------------------------");
    println!(
        "Total Wall-Clock Time:           {:>8.2} ms",
        total_elapsed.as_secs_f64() * 1000.0
    );
    let throughput = if index_elapsed.as_secs_f64() > 0.0 {
        workspace_index.files.len() as f64 / index_elapsed.as_secs_f64()
    } else {
        0.0
    };
    println!(
        "Indexing Throughput:             {:.1} files/sec",
        throughput
    );
}

fn print_archive_rankings(metrics: &mut [ArchiveProcessingMetric]) {
    metrics.sort_by_key(|metric| std::cmp::Reverse(metric.duration));
    println!("\n--- Top 5 Slowest Archives to Index ---");
    for (index, metric) in metrics.iter().take(5).enumerate() {
        let name = metric
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let kind = if metric.is_aar { "AAR" } else { "JAR" };
        println!(
            "  {}. [{}] {:<40} {:>8.2} ms ({} symbols)",
            index + 1,
            kind,
            name,
            metric.duration.as_secs_f64() * 1000.0,
            metric.symbol_count
        );
    }

    metrics.sort_by_key(|metric| std::cmp::Reverse(metric.symbol_count));
    println!("\n--- Top 5 Largest Archives by Symbol Count ---");
    for (index, metric) in metrics.iter().take(5).enumerate() {
        let name = metric
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let kind = if metric.is_aar { "AAR" } else { "JAR" };
        println!(
            "  {}. [{}] {:<40} {:>6} symbols in {:>7.2} ms",
            index + 1,
            kind,
            name,
            metric.symbol_count,
            metric.duration.as_secs_f64() * 1000.0
        );
    }
}

fn print_workspace_stats(workspace_index: &WorkspaceIndex) {
    let total_decls: usize = workspace_index
        .files
        .values()
        .map(|file| file.declarations.len())
        .sum();
    println!("\n--- Final Workspace Index Statistics ---");
    println!(
        "Total Indexed Archive Files:     {}",
        workspace_index.files.len()
    );
    println!("Total Declarations:              {}", total_decls);
    println!(
        "Unique Symbol Names:             {}",
        workspace_index.name_to_decls.len()
    );
    println!(
        "Unique Containers:               {}",
        workspace_index.container_to_decls.len()
    );
    println!(
        "Supertypes Indexed:              {}",
        workspace_index.class_supertypes.len()
    );
    println!(
        "Type Aliases:                    {}",
        workspace_index.type_aliases.len()
    );
}

fn main() {
    let config = GradleConfig::new();
    let total_start = Instant::now();

    print_benchmark_header(&config);
    let resolution = resolve_catalog_artifacts(&config);

    if resolution.resolved_artifacts.is_empty() {
        println!("\nWarning: No JAR/AAR artifacts could be found in the local Gradle cache.");
        println!(
            "Ensure Gradle has downloaded dependencies into ~/.gradle/caches/modules-2/files-2.1"
        );
        return;
    }

    let (workspace_index, mut metrics, index_elapsed, ins_elapsed) =
        index_and_insert_symbols(&resolution.resolved_artifacts, &config);

    let total_elapsed = total_start.elapsed();
    print_performance_breakdown(
        &resolution,
        index_elapsed,
        ins_elapsed,
        total_elapsed,
        &workspace_index,
    );
    print_archive_rankings(&mut metrics);
    print_workspace_stats(&workspace_index);
}
