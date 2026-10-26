use ahash::AHashMap;
use common::files::resolve_target_files;
use lint::Linter;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
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

struct BenchmarkDataset {
    files: Vec<(PathBuf, Vec<u8>, usize)>,
    total_bytes: usize,
    total_lines: usize,
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

struct SlowFileRecord {
    path: PathBuf,
    duration: Duration,
    bytes: usize,
    lines: usize,
    diagnostics_count: usize,
}

struct BenchmarkResult {
    duration: Duration,
    files_count: usize,
    total_bytes: usize,
    total_lines: usize,
    diagnostics_count: usize,
    error_count: usize,
    warning_count: usize,
    top_rules: Vec<(SmolStr, usize)>,
    slowest_files: Vec<SlowFileRecord>,
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

fn format_number(value: usize) -> String {
    let raw_string = value.to_string();
    let mut formatted_string = String::with_capacity(raw_string.len() + raw_string.len() / 3);
    let remainder_offset = raw_string.len() % 3;
    for (index, character) in raw_string.chars().enumerate() {
        if index > 0 && (index % 3 == remainder_offset || (remainder_offset == 0 && index % 3 == 0))
        {
            formatted_string.push(',');
        }
        formatted_string.push(character);
    }
    formatted_string
}

fn print_result_summary(title: &str, result: &BenchmarkResult, speedup_against: Option<Duration>) {
    let elapsed_ms = result.duration.as_secs_f64() * 1000.0;
    let seconds = result.duration.as_secs_f64().max(0.000_001);
    let files_per_sec = (result.files_count as f64 / seconds).round() as usize;
    let mb_per_sec = (result.total_bytes as f64 / (1024.0 * 1024.0)) / seconds;
    let lines_per_sec = (result.total_lines as f64 / seconds).round() as usize;

    println!("\n--- {title} ---");
    println!("Duration:         {elapsed_ms:.2} ms ({seconds:.3} s)");
    println!(
        "Throughput:       {} files/s | {mb_per_sec:.2} MB/s | {} lines/s",
        format_number(files_per_sec),
        format_number(lines_per_sec),
    );
    println!(
        "Diagnostics:      {} (Errors: {}, Warnings: {})",
        format_number(result.diagnostics_count),
        format_number(result.error_count),
        format_number(result.warning_count),
    );
    if let Some(base_duration) = speedup_against {
        let speedup = base_duration.as_secs_f64() / result.duration.as_secs_f64();
        println!("Parallel Speedup: {speedup:.2}x");
    }
}

fn print_top_rules(frequencies: &[(SmolStr, usize)], total_diagnostics: usize, limit: usize) {
    if frequencies.is_empty() {
        return;
    }
    let displayed_count = limit.min(frequencies.len());
    println!("\n--- Top {displayed_count} Triggered Rules ---");
    let total_float = total_diagnostics.max(1) as f64;
    for (index, (rule, count)) in frequencies.iter().take(limit).enumerate() {
        let percentage = (*count as f64 / total_float) * 100.0;
        println!(
            "{:>3}. {:<40} {:>8} ({:5.1}%)",
            index + 1,
            rule,
            format_number(*count),
            percentage
        );
    }
}

fn print_slowest_files(slowest_files: &[SlowFileRecord], target_dir: &Path, limit: usize) {
    if slowest_files.is_empty() {
        return;
    }
    let displayed_count = limit.min(slowest_files.len());
    println!("\n--- Top {displayed_count} Slowest Files ---");
    for (index, record) in slowest_files.iter().take(limit).enumerate() {
        let relative_path = record.path.strip_prefix(target_dir).unwrap_or(&record.path);
        let duration_ms = record.duration.as_secs_f64() * 1000.0;
        let kb_size = record.bytes as f64 / 1024.0;
        println!(
            "{:>2}. {:<56} {:>6.2} ms ({:>5} lines, {:>5.1} KB, {:>3} issues)",
            index + 1,
            relative_path.display(),
            duration_ms,
            format_number(record.lines),
            kb_size,
            format_number(record.diagnostics_count),
        );
    }
}

fn main() {
    if env::args().any(|argument| argument == "--help" || argument == "-h") {
        println!(
            "Usage: cargo bench --bench lint_bench [-- <path-to-kotlin-repo>] [--sequential-only]"
        );
        return;
    }

    let sequential_only = env::args().any(|argument| argument == "--sequential-only");

    let target_dir = env::args()
        .skip(1)
        .find(|argument| !argument.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("benches/kotlinx.coroutines"));

    let resolved_dir = std::fs::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());
    println!("=== Kotlin Static Linter Benchmark ===");
    println!("Target directory: {}", resolved_dir.display());

    let dataset = match load_dataset(&resolved_dir) {
        Ok(dataset) => dataset,
        Err(error) => {
            eprintln!("Failed to load dataset: {error}");
            std::process::exit(1);
        }
    };

    let mb_size = dataset.total_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "Files loaded:     {} Kotlin source files",
        format_number(dataset.files.len())
    );
    println!(
        "Total size:       {:.2} MB ({} bytes)",
        mb_size,
        format_number(dataset.total_bytes)
    );
    println!(
        "Total lines:      {} lines",
        format_number(dataset.total_lines)
    );

    let linter = Linter::standard();
    println!(
        "Enabled rules:    {} active AST rules in standard ruleset",
        linter.registry().ast_rules.len()
    );

    let sequential_result = run_sequential_lint(&linter, &dataset);
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
        print_slowest_files(&sequential_result.slowest_files, &resolved_dir, 5);
        return;
    }

    let parallel_result = run_parallel_lint(&linter, &dataset);
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

    print_slowest_files(&sequential_result.slowest_files, &resolved_dir, 5);
}
