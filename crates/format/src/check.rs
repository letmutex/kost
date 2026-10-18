//! Multi-file check mode runner and outcome evaluation for `kost fmt --check`.

use crate::cli::{determine_file_type, resolve_file_formatter};
use crate::error::FormatError;
use crate::options::StylePreset;
use crate::smart_diff::{render_smart_diffs, resolve_display_tier};
use cli::PrintMode;
use common::editorconfig::EditorConfigResolver;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::io::Write;
use std::path::{Path, PathBuf};

pub enum CheckOutcome {
    Formatted,
    Unformatted {
        path_label: String,
        diff: Option<String>,
    },
    ParseError {
        path_label: String,
        position: crate::error::Position,
        message: String,
    },
    Error(String),
}

pub fn check_file(
    base_preset: StylePreset,
    resolver: Option<&EditorConfigResolver>,
    path: &Path,
    colorize: bool,
) -> CheckOutcome {
    let Some(formatter) = resolve_file_formatter(base_preset, resolver, path) else {
        return CheckOutcome::Formatted;
    };
    let content = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            let path_str = path.display().to_string();
            let err_str = err.to_string();
            return CheckOutcome::Error(crate::concat_str!(
                "Error reading ",
                &path_str,
                ": ",
                &err_str
            ));
        }
    };
    let file_type = determine_file_type(path);
    let path_label = path.to_string_lossy();
    match formatter.check(&content, file_type, &path_label, colorize) {
        Ok(check_result) => {
            if !check_result.is_formatted {
                CheckOutcome::Unformatted {
                    path_label: path_label.into_owned(),
                    diff: check_result.diff,
                }
            } else {
                CheckOutcome::Formatted
            }
        }
        Err(FormatError::ParseError { position, message }) => CheckOutcome::ParseError {
            path_label: path_label.into_owned(),
            position,
            message,
        },
        Err(FormatError::InternalError(err_str)) => {
            let path_str = path.display().to_string();
            CheckOutcome::Error(crate::concat_str!(
                "Error checking ",
                &path_str,
                ": ",
                &err_str
            ))
        }
    }
}

struct PartitionedOutcomes<'a> {
    unformatted_files: Vec<(&'a str, Option<&'a str>)>,
    blocking_error_count: usize,
}

fn partition_check_outcomes(
    outcomes: &[CheckOutcome],
    reject_parse_errors: bool,
) -> PartitionedOutcomes<'_> {
    let mut unformatted_files = Vec::new();
    let mut parse_error_count = 0;
    let mut io_error_count = 0;

    for outcome in outcomes {
        match outcome {
            CheckOutcome::Formatted => {}
            CheckOutcome::Unformatted { path_label, diff } => {
                unformatted_files.push((path_label.as_str(), diff.as_deref()));
            }
            CheckOutcome::ParseError {
                path_label,
                position,
                message,
            } => {
                eprintln!(
                    "Error checking {path_label}: Parse error at line {}, column {}: {message}",
                    position.line, position.column
                );
                parse_error_count += 1;
            }
            CheckOutcome::Error(err_msg) => {
                eprintln!("{err_msg}");
                io_error_count += 1;
            }
        }
    }

    let blocking_error_count = io_error_count
        + if reject_parse_errors {
            parse_error_count
        } else {
            0
        };

    PartitionedOutcomes {
        unformatted_files,
        blocking_error_count,
    }
}

fn build_check_result(
    unformatted_count: usize,
    blocking_error_count: usize,
    duration: std::time::Duration,
) -> Result<(), String> {
    if blocking_error_count > 0 && unformatted_count > 0 {
        let errors_string = blocking_error_count.to_string();
        let count_string = unformatted_count.to_string();
        let elapsed_str = common::format_elapsed(duration);
        Err(crate::concat_str!(
            "Formatting check failed: ",
            &count_string,
            " file(s) need formatting and ",
            &errors_string,
            " error(s) encountered (in ",
            &elapsed_str,
            ")"
        ))
    } else if blocking_error_count > 0 {
        let errors_string = blocking_error_count.to_string();
        let elapsed_str = common::format_elapsed(duration);
        Err(crate::concat_str!(
            "Formatting check failed with ",
            &errors_string,
            " error(s) (in ",
            &elapsed_str,
            ")"
        ))
    } else if unformatted_count > 0 {
        let count_string = unformatted_count.to_string();
        let elapsed_str = common::format_elapsed(duration);
        Err(crate::concat_str!(
            "Formatting check failed: ",
            &count_string,
            " file(s) need formatting (in ",
            &elapsed_str,
            ")"
        ))
    } else {
        Ok(())
    }
}

pub fn run_check_mode(
    base_preset: StylePreset,
    resolver: Option<&EditorConfigResolver>,
    target_files: &[PathBuf],
    print_mode: PrintMode,
    reject_parse_errors: bool,
) -> Result<(), String> {
    let start_time = std::time::Instant::now();
    let colorize = common::should_colorize_stdout();
    let outcomes: Vec<CheckOutcome> = target_files
        .par_iter()
        .map(|path| check_file(base_preset, resolver, path, colorize))
        .collect();

    let partitioned = partition_check_outcomes(&outcomes, reject_parse_errors);
    let unformatted_count = partitioned.unformatted_files.len();

    if unformatted_count > 0 {
        let tier = resolve_display_tier(print_mode, unformatted_count);
        let stdout = std::io::stdout();
        let mut writer = std::io::BufWriter::new(stdout.lock());
        render_smart_diffs(&mut writer, tier, &partitioned.unformatted_files, colorize)
            .map_err(|err| err.to_string())?;
        writer.flush().map_err(|err| err.to_string())?;
    }

    build_check_result(
        unformatted_count,
        partitioned.blocking_error_count,
        start_time.elapsed(),
    )
}
