use crate::{
    BenchmarkDataset, FormatCheckResult, FormatResult, SlowFileRecord, run_parallel_format,
};
use format::{Formatter, StylePreset};
use std::path::Path;
use std::time::Duration;

pub fn format_number(value: usize) -> String {
    let raw = value.to_string();
    let mut result = String::with_capacity(raw.len() + raw.len() / 3);
    let rem = raw.len() % 3;
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && (i % 3 == rem || (rem == 0 && i % 3 == 0)) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

pub fn print_format_summary(
    title: &str,
    result: &FormatResult,
    dataset: &BenchmarkDataset,
    speedup_against: Option<Duration>,
) {
    let seconds = result.duration.as_secs_f64().max(0.000_001);
    let files_per_sec = (dataset.files.len() as f64 / seconds).round() as usize;
    let mb_per_sec = (dataset.total_bytes as f64 / (1024.0 * 1024.0)) / seconds;
    let lines_per_sec = (dataset.total_lines as f64 / seconds).round() as usize;

    println!("\n--- {title} ---");
    println!(
        "Duration:         {:.2} ms ({seconds:.3} s)\n\
         Throughput:       {} files/s | {mb_per_sec:.2} MB/s | {} lines/s\n\
         Results:          {} clean, {} formatted, {} errors",
        result.duration.as_secs_f64() * 1000.0,
        format_number(files_per_sec),
        format_number(lines_per_sec),
        format_number(result.clean_count),
        format_number(result.modified_count),
        format_number(result.error_count),
    );
    if let Some(base_duration) = speedup_against {
        let speedup = base_duration.as_secs_f64() / result.duration.as_secs_f64();
        println!("Parallel Speedup: {speedup:.2}x");
    }
}

pub fn print_check_summary(
    title: &str,
    result: &FormatCheckResult,
    dataset: &BenchmarkDataset,
    speedup_against: Option<Duration>,
) {
    let check_seconds = result.duration.as_secs_f64().max(0.000_001);
    let check_lines_per_sec = (dataset.total_lines as f64 / check_seconds).round() as usize;

    println!("\n--- {title} ---");
    println!(
        "Duration:         {:.2} ms ({check_seconds:.3} s)\n\
         Throughput:       {} lines/s ({} clean, {} need format, {} errors, {} diffs)",
        result.duration.as_secs_f64() * 1000.0,
        format_number(check_lines_per_sec),
        format_number(result.clean_count),
        format_number(result.unformatted_count),
        format_number(result.error_count),
        format_number(result.diff_count),
    );
    if let Some(base_duration) = speedup_against {
        let speedup = base_duration.as_secs_f64() / result.duration.as_secs_f64();
        println!("Parallel Speedup: {speedup:.2}x");
    }
}

pub fn print_slowest_files(slowest_files: &[SlowFileRecord], target_dir: &Path) {
    if slowest_files.is_empty() {
        return;
    }
    println!(
        "\n--- Top {} Slowest Files to Format ---",
        slowest_files.len()
    );
    for (index, record) in slowest_files.iter().enumerate() {
        let relative_path = record.path.strip_prefix(target_dir).unwrap_or(&record.path);
        println!(
            "{:>2}. {:<56} {:>6.2} ms ({:>5} lines, {:>5.1} KB, status: {})",
            index + 1,
            relative_path.display(),
            record.duration.as_secs_f64() * 1000.0,
            format_number(record.lines),
            record.bytes as f64 / 1024.0,
            record.status,
        );
    }
}

pub fn run_preset_comparison(dataset: &BenchmarkDataset) {
    println!("\n--- Style Presets Comparison (Rayon Parallel) ---");
    let presets = [
        ("Kotlinlang (default)", StylePreset::Kotlinlang),
        ("Meta", StylePreset::Meta),
        ("Google", StylePreset::Google),
    ];

    for (label, preset) in presets {
        let formatter = Formatter::from_preset(preset);
        let result = run_parallel_format(&formatter, dataset);
        let seconds = result.duration.as_secs_f64().max(0.000_001);
        let mb_per_sec = (dataset.total_bytes as f64 / (1024.0 * 1024.0)) / seconds;
        let lines_per_sec = (dataset.total_lines as f64 / seconds).round() as usize;

        println!(
            "{:<22} {:>7.2} ms | {:>7} lines/s | {:>6.2} MB/s ({} clean, {} formatted, {} errors)",
            label,
            result.duration.as_secs_f64() * 1000.0,
            format_number(lines_per_sec),
            mb_per_sec,
            format_number(result.clean_count),
            format_number(result.modified_count),
            format_number(result.error_count),
        );
    }
}
