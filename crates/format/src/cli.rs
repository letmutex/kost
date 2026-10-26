//! CLI execution runner for `kost fmt`.

use crate::check::run_check_mode;
use crate::formatter::Formatter;
use crate::options::{FileType, StylePreset};
use crate::smart_diff::{render_smart_diffs, resolve_display_tier};
use cli::{FormatArgs, PrintMode, render_format_help};
use common::resolve_target_files;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Executes the `fmt` command given CLI arguments.
pub fn run(args: &[String]) -> Result<(), String> {
    let parsed_args = FormatArgs::parse(args).map_err(|err| err.to_string())?;

    if parsed_args.help {
        print!("{}", render_format_help());
        return Ok(());
    }

    if parsed_args.targets.len() > 1 && parsed_args.targets.iter().any(|target| target == "-") {
        return Err("Cannot mix standard input '-' with file targets".to_string());
    }

    let preset = StylePreset::from(parsed_args.style);
    let resolver = if parsed_args.editorconfig_enabled {
        Some(
            common::editorconfig::EditorConfigResolver::new_with_fallback(
                parsed_args.editorconfig_path.as_deref(),
            ),
        )
    } else {
        None
    };

    if parsed_args.targets.len() == 1 && parsed_args.targets[0] == "-" {
        let stdin_path = parsed_args.stdin_path.as_deref();
        let stdin_formatter = resolve_file_formatter(
            preset,
            resolver.as_ref(),
            Path::new(stdin_path.unwrap_or("<stdin>")),
        );
        return run_stdin(
            stdin_formatter.as_ref(),
            parsed_args.check,
            stdin_path,
            parsed_args.print_mode,
            parsed_args.reject_parse_errors,
        );
    }

    let target_files = resolve_target_files(&parsed_args.targets).map_err(|err| err.to_string())?;
    if target_files.is_empty() {
        if !parsed_args.targets.is_empty() {
            return Err("No matching Kotlin source files found for specified targets".to_string());
        }
        return Ok(());
    }

    if parsed_args.check {
        run_check_mode(
            preset,
            resolver.as_ref(),
            &target_files,
            parsed_args.print_mode,
            parsed_args.reject_parse_errors,
        )
    } else {
        run_inplace_mode(preset, resolver.as_ref(), &target_files)
    }
}

fn run_stdin(
    formatter: Option<&Formatter>,
    check_mode: bool,
    stdin_path: Option<&str>,
    print_mode: PrintMode,
    reject_parse_errors: bool,
) -> Result<(), String> {
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|err| err.to_string())?;

    let Some(formatter) = formatter else {
        if !check_mode {
            print!("{buffer}");
        }
        return Ok(());
    };

    let file_type = if buffer.starts_with("#!") {
        FileType::Script
    } else if let Some(path_str) = stdin_path {
        determine_file_type(Path::new(path_str))
    } else {
        FileType::Regular
    };

    let start_time = std::time::Instant::now();
    let display_path = stdin_path.unwrap_or("<stdin>");
    if check_mode {
        let colorize = common::should_colorize_stdout();
        match formatter.check(&buffer, file_type, display_path, colorize) {
            Ok(check_result) => {
                if !check_result.is_formatted {
                    let tier = resolve_display_tier(print_mode, 1);
                    let unformatted = [(display_path, check_result.diff.as_deref())];
                    render_smart_diffs(&mut std::io::stdout(), tier, &unformatted, colorize)
                        .map_err(|err| err.to_string())?;
                    let elapsed_str = common::format_elapsed(start_time.elapsed());
                    return Err(common::concat_str!(
                        "Standard input is not formatted (in ",
                        &elapsed_str,
                        ")"
                    ));
                }
            }
            Err(err @ crate::error::FormatError::ParseError { .. }) => {
                let err_str = err.to_string();
                let err_msg = common::concat_str!("Error checking ", display_path, ": ", &err_str);
                if reject_parse_errors {
                    return Err(err_msg);
                }
                eprintln!("{err_msg}");
            }
            Err(err) => return Err(err.to_string()),
        }
    } else {
        let formatted = formatter
            .format_file(&buffer, file_type)
            .map_err(|err| err.to_string())?;
        print!("{formatted}");
    }

    Ok(())
}

pub(crate) fn determine_file_type(path: &Path) -> FileType {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("kts"))
    {
        FileType::Script
    } else {
        FileType::Regular
    }
}

pub(crate) fn resolve_file_formatter(
    base_preset: StylePreset,
    resolver: Option<&common::editorconfig::EditorConfigResolver>,
    path: &Path,
) -> Option<Formatter> {
    if let Some(r) = resolver {
        let resolved = r.resolve_for_path(path);
        if !resolved.is_formatting_enabled() {
            return None;
        }
        let options = crate::options::FormattingOptions::from_editorconfig(base_preset, &resolved);
        Some(Formatter::with_options(options))
    } else {
        Some(Formatter::from_preset(base_preset))
    }
}

enum InplaceOutcome {
    Unchanged,
    Modified(PathBuf),
    Error(String),
}

fn format_file_inplace(
    base_preset: StylePreset,
    resolver: Option<&common::editorconfig::EditorConfigResolver>,
    path: &Path,
) -> InplaceOutcome {
    let Some(formatter) = resolve_file_formatter(base_preset, resolver, path) else {
        return InplaceOutcome::Unchanged;
    };
    let content = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            let path_str = path.display().to_string();
            let err_str = err.to_string();
            return InplaceOutcome::Error(common::concat_str!(
                "Error reading ",
                &path_str,
                ": ",
                &err_str
            ));
        }
    };
    let file_type = determine_file_type(path);
    match formatter.format_file(&content, file_type) {
        Ok(formatted) if formatted != content => match std::fs::write(path, formatted) {
            Ok(()) => InplaceOutcome::Modified(path.to_path_buf()),
            Err(err) => {
                let path_str = path.display().to_string();
                let err_str = err.to_string();
                InplaceOutcome::Error(common::concat_str!(
                    "Error writing ",
                    &path_str,
                    ": ",
                    &err_str
                ))
            }
        },
        Ok(_) => InplaceOutcome::Unchanged,
        Err(err) => {
            let path_str = path.display().to_string();
            let err_str = err.to_string();
            InplaceOutcome::Error(common::concat_str!(
                "Error formatting ",
                &path_str,
                ": ",
                &err_str
            ))
        }
    }
}

fn run_inplace_mode(
    base_preset: StylePreset,
    resolver: Option<&common::editorconfig::EditorConfigResolver>,
    target_files: &[PathBuf],
) -> Result<(), String> {
    let outcomes: Vec<InplaceOutcome> = target_files
        .par_iter()
        .map(|path| format_file_inplace(base_preset, resolver, path))
        .collect();

    let mut error_count = 0;
    for outcome in outcomes {
        match outcome {
            InplaceOutcome::Unchanged => {}
            InplaceOutcome::Modified(path) => {
                println!("Formatted {}", path.display());
            }
            InplaceOutcome::Error(err_msg) => {
                eprintln!("{err_msg}");
                error_count += 1;
            }
        }
    }

    if error_count > 0 {
        let errors_string = error_count.to_string();
        Err(common::concat_str!(
            "Formatting completed with ",
            &errors_string,
            " error(s)"
        ))
    } else {
        Ok(())
    }
}
