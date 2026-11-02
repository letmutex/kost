#[path = "lint_bench/reporting.rs"]
mod reporting;

use ahash::AHashMap;
use common::files::resolve_target_files;
use lint::Linter;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use reporting::{
    print_benchmark_header, print_result_summary, print_slowest_files, print_top_rules,
};
use smol_str::SmolStr;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let newline_count = bytes.iter().filter(|&&byte| byte == b'\n').count();
    newline_count + usize::from(!bytes.ends_with(b"\n"))
}

pub struct BenchmarkDataset {
    pub files: Vec<(PathBuf, Vec<u8>, usize)>,
    pub total_bytes: usize,
    pub total_lines: usize,
}

fn load_dataset(target_dir: &Path) -> Result<BenchmarkDataset, std::io::Error> {
    let target_str = target_dir.to_string_lossy().to_string();
    let file_paths = resolve_target_files(&[target_str])?;

    let files: Vec<(PathBuf, Vec<u8>, usize)> = file_paths
        .into_par_iter()
        .filter_map(|path| {
            let bytes = std::fs::read(&path).ok()?;
            let line_count = count_lines(&bytes);
            Some((path, bytes, line_count))
        })
        .collect();

    let total_bytes = files.iter().map(|(_, bytes, _)| bytes.len()).sum();
    let total_lines = files.iter().map(|(_, _, line_count)| *line_count).sum();

    Ok(BenchmarkDataset {
        files,
        total_bytes,
        total_lines,
    })
}

pub struct SlowFileRecord {
    pub path: PathBuf,
    pub duration: Duration,
    pub bytes: usize,
    pub lines: usize,
    pub diagnostics_count: usize,
}

pub struct BenchmarkResult {
    pub duration: Duration,
    pub files_count: usize,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub diagnostics_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub top_rules: Vec<(SmolStr, usize)>,
    pub slowest_files: Vec<SlowFileRecord>,
}

fn run_sequential_lint(linter: &Linter, dataset: &BenchmarkDataset) -> BenchmarkResult {
    let start_time = Instant::now();
    let mut diagnostics_count = 0;
    let mut error_count = 0;
    let mut warning_count = 0;
    let mut rule_frequencies: AHashMap<SmolStr, usize> = AHashMap::new();
    let mut file_timings = Vec::with_capacity(dataset.files.len());

    for (path, bytes, lines) in &dataset.files {
        let file_start = Instant::now();
        let path_str = path.to_string_lossy();
        let report = linter.lint_source_with_path(bytes, Some(&path_str));
        let file_duration = file_start.elapsed();

        let file_diagnostics = report.diagnostics().len();
        diagnostics_count += file_diagnostics;
        error_count += report.error_count();
        warning_count += report.warning_count();
        for diagnostic in report.diagnostics() {
            *rule_frequencies
                .entry(diagnostic.rule_id.clone())
                .or_insert(0) += 1;
        }

        file_timings.push(SlowFileRecord {
            path: path.clone(),
            duration: file_duration,
            bytes: bytes.len(),
            lines: *lines,
            diagnostics_count: file_diagnostics,
        });
    }

    let duration = start_time.elapsed();
    let mut top_rules: Vec<(SmolStr, usize)> = rule_frequencies.into_iter().collect();
    top_rules.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    file_timings.sort_by_key(|record| std::cmp::Reverse(record.duration));
    file_timings.truncate(5);

    BenchmarkResult {
        duration,
        files_count: dataset.files.len(),
        total_bytes: dataset.total_bytes,
        total_lines: dataset.total_lines,
        diagnostics_count,
        error_count,
        warning_count,
        top_rules,
        slowest_files: file_timings,
    }
}

fn run_parallel_lint(linter: &Linter, dataset: &BenchmarkDataset) -> BenchmarkResult {
    let start_time = Instant::now();

    let (diagnostics_count, error_count, warning_count, rule_frequencies) = dataset
        .files
        .par_iter()
        .fold(
            || (0usize, 0usize, 0usize, AHashMap::<SmolStr, usize>::new()),
            |mut accumulator, (path, bytes, _)| {
                let path_str = path.to_string_lossy();
                let report = linter.lint_source_with_path(bytes, Some(&path_str));
                accumulator.0 += report.diagnostics().len();
                accumulator.1 += report.error_count();
                accumulator.2 += report.warning_count();
                for diagnostic in report.diagnostics() {
                    *accumulator.3.entry(diagnostic.rule_id.clone()).or_insert(0) += 1;
                }
                accumulator
            },
        )
        .reduce(
            || (0usize, 0usize, 0usize, AHashMap::new()),
            |mut left, right| {
                left.0 += right.0;
                left.1 += right.1;
                left.2 += right.2;
                for (rule, count) in right.3 {
                    *left.3.entry(rule).or_insert(0) += count;
                }
                left
            },
        );

    let duration = start_time.elapsed();
    let mut top_rules: Vec<(SmolStr, usize)> = rule_frequencies.into_iter().collect();
    top_rules.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    BenchmarkResult {
        duration,
        files_count: dataset.files.len(),
        total_bytes: dataset.total_bytes,
        total_lines: dataset.total_lines,
        diagnostics_count,
        error_count,
        warning_count,
        top_rules,
        slowest_files: Vec::new(),
    }
}

fn parse_cli_options() -> Option<(PathBuf, bool)> {
    if env::args().any(|argument| argument == "--help" || argument == "-h") {
        println!(
            "Usage: cargo bench --bench lint_bench [-- <path-to-kotlin-repo>] [--sequential-only]"
        );
        return None;
    }

    let sequential_only = env::args().any(|argument| argument == "--sequential-only");
    let target_dir = env::args()
        .skip(1)
        .find(|argument| !argument.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("benches/kotlinx.coroutines"));

    Some((target_dir, sequential_only))
}

fn run_benchmark_suite(
    linter: &Linter,
    dataset: &BenchmarkDataset,
    resolved_dir: &Path,
    sequential_only: bool,
) {
    let sequential_result = run_sequential_lint(linter, dataset);
    let title = if sequential_only {
        "In-Memory Sequential Linting (Single Thread)"
    } else {
        "1. In-Memory Sequential Linting (Single Thread)"
    };
    print_result_summary(title, &sequential_result, None);

    if sequential_only {
        print_top_rules(
            &sequential_result.top_rules,
            sequential_result.diagnostics_count,
            15,
        );
        print_slowest_files(&sequential_result.slowest_files, resolved_dir, 5);
        return;
    }

    let parallel_result = run_parallel_lint(linter, dataset);
    print_result_summary(
        "2. In-Memory Parallel Linting (Rayon Multi-Threaded)",
        &parallel_result,
        Some(sequential_result.duration),
    );

    print_top_rules(
        &parallel_result.top_rules,
        parallel_result.diagnostics_count,
        15,
    );
    print_slowest_files(&sequential_result.slowest_files, resolved_dir, 5);
}

fn main() {
    let Some((target_dir, sequential_only)) = parse_cli_options() else {
        return;
    };

    let resolved_dir = std::fs::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());
    let dataset = match load_dataset(&resolved_dir) {
        Ok(dataset) => dataset,
        Err(error) => {
            eprintln!("Failed to load dataset: {error}");
            std::process::exit(1);
        }
    };

    let linter = Linter::standard();
    print_benchmark_header(&resolved_dir, &dataset, linter.registry().ast_rules.len());
    run_benchmark_suite(&linter, &dataset, &resolved_dir, sequential_only);
}
