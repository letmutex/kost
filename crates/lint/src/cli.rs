//! CLI execution runner for `kost lint`.

use crate::linter::Linter;
use cli::{LintArgs, render_lint_help};
use common::resolve_target_files;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Executes the `lint` command given CLI arguments.
pub fn run(args: &[String]) -> Result<(), String> {
    let start_time = std::time::Instant::now();
    let parsed_args = LintArgs::parse(args).map_err(|err| err.to_string())?;

    if parsed_args.help {
        print!("{}", render_lint_help());
        return Ok(());
    }

    let linter = resolve_linter(parsed_args.rulesets.as_deref())?;
    let resolver = if parsed_args.editorconfig_enabled {
        Some(
            common::editorconfig::EditorConfigResolver::new_with_fallback(
                parsed_args.editorconfig_path.as_deref(),
            ),
        )
    } else {
        None
    };

    if parsed_args.targets.len() > 1 && parsed_args.targets.iter().any(|target| target == "-") {
        return Err("Cannot mix standard input '-' with file targets".to_string());
    }

    if parsed_args.targets.len() == 1 && parsed_args.targets[0] == "-" {
        return run_stdin(
            &linter,
            resolver.as_ref(),
            parsed_args.stdin_path.as_deref(),
            parsed_args.print_mode,
            parsed_args.deny_warnings,
        );
    }

    let target_files = resolve_active_targets(&linter, &parsed_args.targets)?;
    if target_files.is_empty() {
        return Ok(());
    }

    let (reports, read_errors) = collect_reports(&linter, resolver.as_ref(), &target_files);
    render_reports(parsed_args.print_mode, &reports)?;

    let error_count: usize = reports
        .iter()
        .map(|report| report.error_count())
        .sum::<usize>()
        + read_errors;
    let warning_count: usize = reports
        .iter()
        .map(|report| report.warning_count())
        .sum::<usize>();
    let total_violations = error_count + warning_count;

    if total_violations > 0 {
        let elapsed_str = common::format_elapsed(start_time.elapsed());
        let summary = format_lint_summary(total_violations, &elapsed_str);
        if error_count > 0 || (parsed_args.deny_warnings && warning_count > 0) {
            Err(summary)
        } else {
            println!("{summary}");
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn resolve_active_targets(linter: &Linter, inputs: &[String]) -> Result<Vec<PathBuf>, String> {
    let files = resolve_target_files(inputs).map_err(|err| err.to_string())?;
    if files.is_empty() && !inputs.is_empty() {
        return Err("No matching Kotlin source files found for specified targets".to_string());
    }
    let filtered: Vec<_> = files
        .into_iter()
        .filter(|path| !linter.is_path_ignored(&path.to_string_lossy()))
        .collect();
    Ok(filtered)
}

fn render_reports(
    mode: cli::PrintMode,
    reports: &[crate::linter::LintReport],
) -> Result<(), String> {
    if reports.is_empty() {
        return Ok(());
    }
    let colorize = common::should_colorize_stdout();
    let tier = crate::smart_lint::resolve_display_tier(mode, reports.len());
    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());
    crate::smart_lint::render_smart_lint_reports(&mut writer, tier, reports, colorize)
        .map_err(|err| err.to_string())?;
    writer.flush().map_err(|err| err.to_string())
}

fn format_lint_summary(total_violations: usize, elapsed_str: &str) -> String {
    let violations_str = total_violations.to_string();
    common::concat_str!(
        "Lint completed with ",
        &violations_str,
        " issue(s) (in ",
        elapsed_str,
        ")"
    )
}

fn collect_reports(
    linter: &Linter,
    resolver: Option<&common::editorconfig::EditorConfigResolver>,
    target_files: &[std::path::PathBuf],
) -> (Vec<crate::linter::LintReport>, usize) {
    let outcomes: Vec<Result<Option<crate::linter::LintReport>, String>> = target_files
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy();
            let bytes = match std::fs::read(path) {
                Ok(file_bytes) => file_bytes,
                Err(err) => {
                    let err_str = err.to_string();
                    return Err(common::concat_str!(
                        "Error reading ",
                        &path_str,
                        ": ",
                        &err_str
                    ));
                }
            };

            let editorconfig =
                resolver.map(|config_resolver| config_resolver.resolve_for_path(path));
            let report = linter.lint_source_with_config(
                &bytes,
                Some(path_str.as_ref()),
                editorconfig.as_ref(),
            );
            if !report.is_clean() {
                Ok(Some(report))
            } else {
                Ok(None)
            }
        })
        .collect();

    let mut reports = Vec::new();
    let mut read_errors = 0;
    for outcome in outcomes {
        match outcome {
            Ok(Some(report)) => reports.push(report),
            Ok(None) => {}
            Err(err_msg) => {
                eprintln!("{err_msg}");
                read_errors += 1;
            }
        }
    }

    (reports, read_errors)
}

fn resolve_linter(rulesets_opt: Option<&str>) -> Result<Linter, String> {
    if let Some(rulesets_str) = rulesets_opt {
        let names: Vec<&str> = rulesets_str
            .split(',')
            .map(|ruleset_name| ruleset_name.trim())
            .filter(|ruleset_name| !ruleset_name.is_empty())
            .collect();
        Linter::from_rulesets(&names).map_err(|err| err.to_string())
    } else if Path::new("kost.yml").is_file() {
        let registry = crate::config::ConfigLoader::load_file(Path::new("kost.yml"))
            .map_err(|err| err.to_string())?;
        Ok(Linter::from_registry(registry))
    } else if Path::new("kost.yaml").is_file() {
        let registry = crate::config::ConfigLoader::load_file(Path::new("kost.yaml"))
            .map_err(|err| err.to_string())?;
        Ok(Linter::from_registry(registry))
    } else {
        Ok(Linter::standard())
    }
}

fn run_stdin(
    linter: &Linter,
    resolver: Option<&common::editorconfig::EditorConfigResolver>,
    stdin_path: Option<&str>,
    mode: cli::PrintMode,
    deny_warnings: bool,
) -> Result<(), String> {
    let start_time = std::time::Instant::now();
    let mut stdin_bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut stdin_bytes)
        .map_err(|err| err.to_string())?;

    let virtual_path = stdin_path.unwrap_or("<stdin>");
    if virtual_path != "<stdin>" && linter.is_path_ignored(virtual_path) {
        return Ok(());
    }

    let editorconfig =
        resolver.map(|config_resolver| config_resolver.resolve_for_path(Path::new(virtual_path)));
    let report =
        linter.lint_source_with_config(&stdin_bytes, Some(virtual_path), editorconfig.as_ref());
    let error_count = report.error_count();
    let warning_count = report.warning_count();
    let violations = error_count + warning_count;

    if !report.is_clean() {
        let colorize = common::should_colorize_stdout();
        let tier = crate::smart_lint::resolve_display_tier(mode, 1);
        let stdout = std::io::stdout();
        let mut writer = std::io::BufWriter::new(stdout.lock());
        crate::smart_lint::render_smart_lint_reports(
            &mut writer,
            tier,
            std::slice::from_ref(&report),
            colorize,
        )
        .map_err(|err| err.to_string())?;
        writer.flush().map_err(|err| err.to_string())?;
    }

    if violations > 0 {
        let elapsed_str = common::format_elapsed(start_time.elapsed());
        let summary = format_lint_summary(violations, &elapsed_str);
        if error_count > 0 || (deny_warnings && warning_count > 0) {
            return Err(summary);
        } else {
            println!("{summary}");
            return Ok(());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Linter, Path, resolve_active_targets, resolve_linter, run};

    #[test]
    fn test_resolve_linter_default_standard() {
        let linter = resolve_linter(None).unwrap();
        assert!(linter.registry().ast_dispatch.contains_key(";"));
    }

    #[test]
    fn test_resolve_linter_explicit_rulesets() {
        let linter = resolve_linter(Some("standard,statistics")).unwrap();
        assert!(linter.registry().ast_dispatch.contains_key(";"));
        assert!(
            linter
                .registry()
                .stat_dispatch
                .contains_key(&crate::types::Scope::Function)
        );
    }

    #[test]
    fn test_cli_run_help_flag() {
        let result = run(&["--help".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_run_unknown_option_fails() {
        let result = run(&["--unknown-flag".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_run_file_with_violations_returns_error() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases/no_semi/duplicateSemicolonsSameLine.kt");
        let result = run(&[
            "-D".to_string(),
            "warnings".to_string(),
            fixture_path.to_string_lossy().to_string(),
        ]);
        let err = result.expect_err("Should fail with violations when denying warnings");
        assert!(err.contains("Lint completed with"));
    }

    #[test]
    fn test_cli_run_file_with_warnings_only_succeeds_without_deny_flag() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases/no_semi/duplicateSemicolonsSameLine.kt");
        let result = run(&[fixture_path.to_string_lossy().to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_run_explicit_target_missing_kotlin_files_fails() {
        let non_kotlin_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let result = run(&[non_kotlin_file.to_string_lossy().to_string()]);
        let err = result.expect_err("Should fail when explicit target has no Kotlin files");
        assert!(err.contains("No matching Kotlin source files"));
    }

    #[test]
    fn test_cli_ignores_files_matching_config_ignore() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases/no_semi/duplicateSemicolonsSameLine.kt");
        let yaml = r#"
lint:
  imports:
    - standard
  ignore:
    - "**/duplicateSemicolonsSameLine.kt"
"#;
        let registry = crate::config::ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
        let linter = Linter::from_registry(registry);
        assert!(linter.is_path_ignored(&fixture_path.to_string_lossy()));
    }

    #[test]
    fn test_resolve_active_targets_succeeds_when_all_targets_are_ignored() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/cases/no_semi/duplicateSemicolonsSameLine.kt");
        let yaml = r#"
lint:
  imports:
    - standard
  ignore:
    - "**/duplicateSemicolonsSameLine.kt"
"#;
        let registry = crate::config::ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
        let linter = Linter::from_registry(registry);
        let targets = [fixture_path.to_string_lossy().to_string()];
        let resolved = resolve_active_targets(&linter, &targets)
            .expect("Should succeed when all targets are ignored");
        assert!(resolved.is_empty());
    }
}
