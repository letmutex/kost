#[path = "format_bench/reporting.rs"]
mod reporting;

use common::files::resolve_target_files;
use format::{FileType, Formatter};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use reporting::{format_number, print_format_summary, print_slowest_files, run_preset_comparison};
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn determine_file_type(path: &Path) -> FileType {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("kts") => FileType::Script,
        _ => FileType::Regular,
    }
}

pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
    pub file_type: FileType,
    pub lines: usize,
    pub bytes: usize,
}

pub struct BenchmarkDataset {
    pub files: Vec<SourceFile>,
    pub total_bytes: usize,
    pub total_lines: usize,
}

pub struct SlowFileRecord {
    pub path: PathBuf,
    pub duration: Duration,
    pub lines: usize,
    pub bytes: usize,
    pub status: &'static str,
}

pub struct FormatResult {
    pub duration: Duration,
    pub clean_count: usize,
    pub modified_count: usize,
    pub error_count: usize,
    pub slowest_files: Vec<SlowFileRecord>,
}

fn load_dataset(target_dir: &Path) -> Result<BenchmarkDataset, std::io::Error> {
    let target_str = target_dir.to_string_lossy().to_string();
    let file_paths = resolve_target_files(&[target_str])?;

    let files: Vec<SourceFile> = file_paths
        .into_par_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            let bytes = content.len();
            let lines = content.lines().count();
            let file_type = determine_file_type(&path);
            Some(SourceFile {
                path,
                content,
                file_type,
                lines,
                bytes,
            })
        })
        .collect();

    let total_bytes = files.iter().map(|file| file.bytes).sum();
    let total_lines = files.iter().map(|file| file.lines).sum();

    Ok(BenchmarkDataset {
        files,
        total_bytes,
        total_lines,
    })
}

fn run_sequential_format(formatter: &Formatter, dataset: &BenchmarkDataset) -> FormatResult {
    let start_time = Instant::now();
    let mut clean_count = 0;
    let mut modified_count = 0;
    let mut error_count = 0;
    let mut file_timings = Vec::with_capacity(dataset.files.len());

    for (file_index, file) in dataset.files.iter().enumerate() {
        let file_start = Instant::now();
        let outcome = formatter.format_file(&file.content, file.file_type);
        let file_duration = file_start.elapsed();

        let status = match outcome {
            Ok(ref formatted) if formatted == &file.content => {
                clean_count += 1;
                "clean"
            }
            Ok(_) => {
                modified_count += 1;
                "modified"
            }
            Err(_) => {
                error_count += 1;
                "error"
            }
        };

        file_timings.push((file_index, file_duration, status));
    }

    let duration = start_time.elapsed();
    file_timings.sort_by_key(|record| std::cmp::Reverse(record.1));
    file_timings.truncate(5);

    let slowest_files = file_timings
        .into_iter()
        .map(|(file_index, file_duration, status)| {
            let file = &dataset.files[file_index];
            SlowFileRecord {
                path: file.path.clone(),
                duration: file_duration,
                lines: file.lines,
                bytes: file.bytes,
                status,
            }
        })
        .collect();

    FormatResult {
        duration,
        clean_count,
        modified_count,
        error_count,
        slowest_files,
    }
}

pub fn run_parallel_format(formatter: &Formatter, dataset: &BenchmarkDataset) -> FormatResult {
    let start_time = Instant::now();

    let (clean_count, modified_count, error_count) = dataset
        .files
        .par_iter()
        .fold(
            || (0usize, 0usize, 0usize),
            |mut accumulator, file| {
                match formatter.format_file(&file.content, file.file_type) {
                    Ok(formatted) if formatted == file.content => accumulator.0 += 1,
                    Ok(_) => accumulator.1 += 1,
                    Err(_) => accumulator.2 += 1,
                }
                accumulator
            },
        )
        .reduce(
            || (0usize, 0usize, 0usize),
            |left, right| (left.0 + right.0, left.1 + right.1, left.2 + right.2),
        );

    FormatResult {
        duration: start_time.elapsed(),
        clean_count,
        modified_count,
        error_count,
        slowest_files: Vec::new(),
    }
}

fn run_parallel_check(
    formatter: &Formatter,
    dataset: &BenchmarkDataset,
) -> (Duration, usize, usize, usize, usize) {
    let start_time = Instant::now();

    let (clean, unformatted, diffs, errors) = dataset
        .files
        .par_iter()
        .fold(
            || (0usize, 0usize, 0usize, 0usize),
            |mut accumulator, file| {
                let path_label = file.path.to_string_lossy();
                match formatter.check(&file.content, file.file_type, &path_label, false) {
                    Ok(result) if result.is_formatted => accumulator.0 += 1,
                    Ok(result) => {
                        accumulator.1 += 1;
                        accumulator.2 += usize::from(result.diff.is_some());
                    }
                    Err(_) => accumulator.3 += 1,
                }
                accumulator
            },
        )
        .reduce(
            || (0usize, 0usize, 0usize, 0usize),
            |left, right| {
                (
                    left.0 + right.0,
                    left.1 + right.1,
                    left.2 + right.2,
                    left.3 + right.3,
                )
            },
        );

    (start_time.elapsed(), clean, unformatted, diffs, errors)
}

fn main() {
    if env::args().any(|argument| argument == "--help" || argument == "-h") {
        println!("Usage: cargo bench --bench format_bench [-- <path-to-kotlin-repo>]");
        return;
    }

    let target_dir = env::args()
        .skip(1)
        .find(|argument| !argument.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("benches/kotlinx.coroutines"));

    let resolved_dir = std::fs::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());
    let dataset = match load_dataset(&resolved_dir) {
        Ok(dataset) => dataset,
        Err(error) => {
            eprintln!("Failed to load dataset: {error}");
            std::process::exit(1);
        }
    };
    let mb_size = dataset.total_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "=== Kotlin Code Formatter Benchmark ===\n\
         Target directory: {}\n\
         Files loaded:     {} Kotlin source files\n\
         Total size:       {mb_size:.2} MB ({} bytes)\n\
         Total lines:      {} lines\n\
         Active style:     Kotlinlang preset (default)",
        resolved_dir.display(),
        format_number(dataset.files.len()),
        format_number(dataset.total_bytes),
        format_number(dataset.total_lines),
    );

    let formatter = Formatter::new();
    let seq_result = run_sequential_format(&formatter, &dataset);
    print_format_summary(
        "1. In-Memory Sequential Formatting (Single Thread)",
        &seq_result,
        &dataset,
        None,
    );

    let par_result = run_parallel_format(&formatter, &dataset);
    print_format_summary(
        "2. In-Memory Parallel Formatting (Rayon Multi-Threaded)",
        &par_result,
        &dataset,
        Some(seq_result.duration),
    );

    let (check_duration, clean, unformatted, diffs, errors) =
        run_parallel_check(&formatter, &dataset);
    let check_seconds = check_duration.as_secs_f64().max(0.000_001);
    let check_lines_per_sec = (dataset.total_lines as f64 / check_seconds).round() as usize;
    println!(
        "\n--- 3. Format Verification & Diff Generation (Rayon Parallel) ---\n\
         Duration:         {:.2} ms ({check_seconds:.3} s)\n\
         Throughput:       {} lines/s ({} clean, {} need format, {} errors, {} diffs)",
        check_duration.as_secs_f64() * 1000.0,
        format_number(check_lines_per_sec),
        format_number(clean),
        format_number(unformatted),
        format_number(errors),
        format_number(diffs),
    );

    print_slowest_files(&seq_result.slowest_files, &resolved_dir);
    run_preset_comparison(&dataset);
}
