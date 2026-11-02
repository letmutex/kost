use crate::{BenchmarkDataset, BenchmarkResult, SlowFileRecord};
use smol_str::SmolStr;
use std::path::Path;
use std::time::Duration;

pub fn format_number(value: usize) -> String {
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

pub fn print_benchmark_header(resolved_dir: &Path, dataset: &BenchmarkDataset, rule_count: usize) {
    let mb_size = dataset.total_bytes as f64 / (1024.0 * 1024.0);
    println!("=== Kotlin Static Linter Benchmark ===");
    println!("Target directory: {}", resolved_dir.display());
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
    println!(
        "Enabled rules:    {} active AST rules in standard ruleset",
        rule_count
    );
}

pub fn print_result_summary(
    title: &str,
    result: &BenchmarkResult,
    speedup_against: Option<Duration>,
) {
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

pub fn print_top_rules(frequencies: &[(SmolStr, usize)], total_diagnostics: usize, limit: usize) {
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

pub fn print_slowest_files(slowest_files: &[SlowFileRecord], target_dir: &Path, limit: usize) {
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
