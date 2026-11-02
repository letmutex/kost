use ahash::AHashSet;
use common::concat_str;
use lsp::WorkspaceIndex;
use lsp::gradle::archive::ArchiveIndexer;
use lsp::gradle::cache_resolver::GradleCacheResolver;
use lsp::gradle::config::GradleConfig;
use lsp::gradle::dependency::deduplicate_newest;
use lsp::gradle::discovery::discover_workspace_pipeline_context;
use lsp::gradle::graph::DependencyGraph;
use lsp::gradle::kotlinc::KotlincResolver;
use lsp::inference::infer_variable_type;
use resolver::ResolvedArtifact;
use std::path::Path;
use std::time::{Duration, Instant};
use tower_lsp::lsp_types::{Position, Url};

const COROUTINES_DIR: &str = "benches/kotlinx.coroutines";

struct BenchmarkTarget {
    label: &'static str,
    file_rel_path: &'static str,
    var_name: &'static str,
    line: u32,
    col: u32,
}

const TARGETS: &[BenchmarkTarget] = &[
    BenchmarkTarget {
        label: "val job = Job()",
        file_rel_path: "kotlinx-coroutines-core/common/test/JobExtensionsTest.kt",
        var_name: "job",
        line: 8,
        col: 16,
    },
    BenchmarkTarget {
        label: "val scope = CoroutineScope(job + ...)",
        file_rel_path: "kotlinx-coroutines-core/common/test/JobExtensionsTest.kt",
        var_name: "scope",
        line: 9,
        col: 16,
    },
    BenchmarkTarget {
        label: "val exception = result... ?: fail()",
        file_rel_path: "kotlinx-coroutines-core/common/test/JobExtensionsTest.kt",
        var_name: "exception",
        line: 77,
        col: 12,
    },
    BenchmarkTarget {
        label: "val first = numbers",
        file_rel_path: "benchmarks/src/jmh/kotlin/benchmarks/flow/NumbersBenchmark.kt",
        var_name: "first",
        line: 63,
        col: 12,
    },
    BenchmarkTarget {
        label: "val me = Channel<Letter>()",
        file_rel_path: "benchmarks/src/jmh/kotlin/benchmarks/scheduler/actors/PingPongActorBenchmark.kt",
        var_name: "me",
        line: 46,
        col: 12,
    },
    BenchmarkTarget {
        label: "val flow = MutableSharedFlow<Unit>()",
        file_rel_path: "kotlinx-coroutines-core/benchmarks/main/kotlin/SharedFlowBaseline.kt",
        var_name: "flow",
        line: 16,
        col: 12,
    },
];

struct BenchmarkResult {
    label: &'static str,
    file_name: &'static str,
    line: u32,
    col: u32,
    inferred_type: Option<String>,
    duration: Duration,
}

fn print_results_summary_table(results: &[BenchmarkResult], total_elapsed: Duration) {
    println!("\n{:=<120}", "");
    println!(
        "{:<38} {:<28} {:>10} {:>12}   {:<28}",
        "Target Variable", "File", "Line:Col", "Time", "Inferred Type"
    );
    println!("{:-<120}", "");

    for result in results {
        let line_str = (result.line + 1).to_string();
        let col_str = (result.col + 1).to_string();
        let loc_tag = concat_str!(&line_str, ":", &col_str);
        let time_str = format!("{:.3} ms", result.duration.as_secs_f64() * 1000.0);
        let type_display = result.inferred_type.as_deref().unwrap_or("(unresolved)");

        println!(
            "{:<38} {:<28} {:>10} {:>12}   {:<28}",
            result.label, result.file_name, loc_tag, time_str, type_display
        );
    }
    println!("{:=<120}", "");

    let avg_per_var = if !results.is_empty() {
        total_elapsed / results.len() as u32
    } else {
        Duration::ZERO
    };

    println!(
        "Total inference time: {:.3} ms | Average per variable: {:.3} ms (across {} targets)\n",
        total_elapsed.as_secs_f64() * 1000.0,
        avg_per_var.as_secs_f64() * 1000.0,
        results.len()
    );
}

fn resolve_dependency_artifacts(
    abs_workspace_dir: &Path,
    config: &GradleConfig,
) -> Vec<ResolvedArtifact> {
    let discovery = discover_workspace_pipeline_context(abs_workspace_dir);
    let deduped_deps = deduplicate_newest(discovery.declared_dependencies);

    let gradle_home = config.resolve_gradle_user_home();
    let graph_result =
        DependencyGraph::resolve_transitive_graph(&deduped_deps, gradle_home.as_deref());

    let mut resolved_artifacts =
        GradleCacheResolver::resolve_all(&graph_result.resolved_dependencies, config);

    let stdlib_artifact = KotlincResolver::find_kotlinc_stdlib(config)
        .or_else(|| GradleCacheResolver::find_kotlin_stdlib(None, config));
    if let Some(stdlib) = stdlib_artifact
        && !resolved_artifacts
            .iter()
            .any(|artifact| artifact.binary_path == stdlib.binary_path)
    {
        resolved_artifacts.push(stdlib);
    }

    resolved_artifacts
}

fn prepare_dependency_archives(
    resolved_artifacts: &[ResolvedArtifact],
    prefer_sources: bool,
) -> Vec<&Path> {
    let mut archives = Vec::new();
    let mut seen = AHashSet::default();
    for artifact in resolved_artifacts {
        let path = if prefer_sources && let Some(ref src) = artifact.sources_path {
            src.as_path()
        } else {
            artifact.binary_path.as_path()
        };
        if seen.insert(path) {
            archives.push(path);
        }
    }
    archives
}

fn index_dependency_jars(index: &mut WorkspaceIndex, abs_workspace_dir: &Path) {
    let config = GradleConfig::new();
    let start = Instant::now();

    let resolved_artifacts = resolve_dependency_artifacts(abs_workspace_dir, &config);
    let archives = prepare_dependency_archives(&resolved_artifacts, config.prefer_sources);

    let (symbols, _) = ArchiveIndexer::index_archives_parallel(&archives, config.prefer_sources);
    let sym_count = symbols.len();
    for (url, file_symbols) in symbols {
        index.insert_file_symbols(url, file_symbols);
    }

    println!(
        "Indexed {} dependency archive(s) ({} symbols) in {:.2} ms",
        archives.len(),
        sym_count,
        start.elapsed().as_secs_f64() * 1000.0,
    );
}

fn index_workspace_directory(abs_workspace_dir: &Path) -> WorkspaceIndex {
    println!("\nIndexing workspace directory...");
    let mut index = WorkspaceIndex::new();
    let index_start = Instant::now();
    index.scan_directory(abs_workspace_dir);
    let index_elapsed = index_start.elapsed();

    let total_files = index.files.len();
    let total_declarations: usize = index
        .files
        .values()
        .map(|file| file.declarations.len())
        .sum();

    println!(
        "Indexed {} files with {} declarations in {:.2} ms",
        total_files,
        total_declarations,
        index_elapsed.as_secs_f64() * 1000.0
    );

    index_dependency_jars(&mut index, abs_workspace_dir);
    index
}

fn benchmark_target(
    index: &WorkspaceIndex,
    abs_workspace_dir: &Path,
    target: &BenchmarkTarget,
) -> Option<BenchmarkResult> {
    let file_path = abs_workspace_dir.join(target.file_rel_path);
    let url = match Url::from_file_path(&file_path) {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Failed to construct Url for: {}", file_path.display());
            return None;
        }
    };

    let source = match index.get_file_source(&url) {
        Some(source) => source,
        None => {
            eprintln!("Source not found for {url}");
            return None;
        }
    };

    let position = Position::new(target.line, target.col);
    let file_name = Path::new(target.file_rel_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(target.file_rel_path);

    print!("Benchmarking {} -> '{}' ... ", file_name, target.label);
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let start = Instant::now();
    let inferred_type = infer_variable_type(index, &url, target.var_name, position, &source, None);
    let duration = start.elapsed();

    let type_string = inferred_type.map(|inferred| inferred.to_string());
    println!(
        "done in {:.3} ms (inferred: {})",
        duration.as_secs_f64() * 1000.0,
        type_string.as_deref().unwrap_or("(unresolved)")
    );

    Some(BenchmarkResult {
        label: target.label,
        file_name,
        line: target.line,
        col: target.col,
        inferred_type: type_string,
        duration,
    })
}

fn run_inference_benchmarks(
    index: &WorkspaceIndex,
    abs_workspace_dir: &Path,
) -> (Vec<BenchmarkResult>, Duration) {
    println!(
        "\nRunning type inference benchmark on {} variable(s)...",
        TARGETS.len()
    );

    let mut results = Vec::new();
    let mut total_duration = Duration::ZERO;

    for target in TARGETS {
        if let Some(bench_result) = benchmark_target(index, abs_workspace_dir, target) {
            total_duration += bench_result.duration;
            results.push(bench_result);
        }
    }

    (results, total_duration)
}

fn main() {
    let workspace_dir = Path::new(COROUTINES_DIR);
    let abs_workspace_dir =
        std::fs::canonicalize(workspace_dir).unwrap_or_else(|_| workspace_dir.to_path_buf());

    println!("===============================================================");
    println!("        TYPE INFERENCE BENCHMARK: KOTLINX.COROUTINES          ");
    println!("===============================================================");
    println!("Target directory: {}", abs_workspace_dir.display());

    if !abs_workspace_dir.exists() {
        eprintln!(
            "Error: directory '{}' does not exist.\nEnsure the submodule is cloned or provide a valid path.",
            abs_workspace_dir.display()
        );
        std::process::exit(1);
    }

    let index = index_workspace_directory(&abs_workspace_dir);
    let (results, total_duration) = run_inference_benchmarks(&index, &abs_workspace_dir);

    if !results.is_empty() {
        print_results_summary_table(&results, total_duration);
    }
}
