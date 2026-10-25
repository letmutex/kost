//! Smart presentation for lint diagnostics across multiple files.

use crate::linter::LintReport;
use crate::types::Diagnostic;
use cli::PrintMode;
use common::color::{BOLD, BOLD_RED, BOLD_YELLOW, CYAN, DIM, RESET, YELLOW};
use std::io::Write;

const MAX_TRUNCATED_DIAGNOSTICS: usize = 20;

/// Display tier for smart lint reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayTier {
    Full,
    Truncated,
    Collapsed,
}

/// Resolves the effective display tier given user mode and number of violated files.
pub fn resolve_display_tier(mode: PrintMode, violated_count: usize) -> DisplayTier {
    match mode {
        PrintMode::Auto => match violated_count {
            0..=10 => DisplayTier::Full,
            11..=20 => DisplayTier::Truncated,
            _ => DisplayTier::Collapsed,
        },
        PrintMode::Full => DisplayTier::Full,
        PrintMode::Truncated => DisplayTier::Truncated,
        PrintMode::Collapsed => DisplayTier::Collapsed,
    }
}

/// Renders smart lint reports for all violated files according to the active display tier.
pub fn render_smart_lint_reports<W: Write>(
    writer: &mut W,
    tier: DisplayTier,
    reports: &[LintReport],
    colorize: bool,
) -> std::io::Result<()> {
    match tier {
        DisplayTier::Full => render_full_reports(writer, reports, colorize),
        DisplayTier::Truncated => render_truncated_reports(writer, reports, colorize),
        DisplayTier::Collapsed => render_collapsed_reports(writer, reports, colorize),
    }
}

fn render_full_reports<W: Write>(
    writer: &mut W,
    reports: &[LintReport],
    colorize: bool,
) -> std::io::Result<()> {
    let mut has_emitted = false;
    let mut last_was_snippet = false;
    for report in reports {
        let path = report.file_path();
        if let Some(source) = report.source() {
            let lines: Vec<&str> = source.lines().collect();
            for diagnostic in report.diagnostics() {
                if has_emitted {
                    writeln!(writer)?;
                }
                render_full_snippet_diagnostic(writer, diagnostic, &lines, path, colorize)?;
                has_emitted = true;
                last_was_snippet = true;
            }
        } else {
            for diagnostic in report.diagnostics() {
                if last_was_snippet {
                    writeln!(writer)?;
                }
                render_diagnostic(writer, diagnostic, path, colorize)?;
                has_emitted = true;
                last_was_snippet = false;
            }
        }
    }
    Ok(())
}

pub use crate::snippet::render_full_snippet_diagnostic;

fn render_truncated_reports<W: Write>(
    writer: &mut W,
    reports: &[LintReport],
    colorize: bool,
) -> std::io::Result<()> {
    for report in reports {
        let path = report.file_path();
        let diagnostics = report.diagnostics();
        let count = diagnostics.len();
        let (slice, is_truncated) = if count > MAX_TRUNCATED_DIAGNOSTICS {
            (&diagnostics[..MAX_TRUNCATED_DIAGNOSTICS], true)
        } else {
            (diagnostics, false)
        };
        for diagnostic in slice {
            render_diagnostic(writer, diagnostic, path, colorize)?;
        }
        if is_truncated {
            let path_label = path.unwrap_or("<input>");
            render_truncated_footer(writer, path_label, colorize)?;
        }
    }
    Ok(())
}

fn render_collapsed_reports<W: Write>(
    writer: &mut W,
    reports: &[LintReport],
    colorize: bool,
) -> std::io::Result<()> {
    if reports.is_empty() {
        return Ok(());
    }
    for report in reports {
        let path_label = report.file_path().unwrap_or("<input>");
        render_collapsed_header(
            writer,
            path_label,
            report.error_count(),
            report.warning_count(),
            colorize,
        )?;
    }
    render_collapsed_footer(writer, reports.len(), colorize)?;
    Ok(())
}

fn render_collapsed_header<W: Write>(
    writer: &mut W,
    path_label: &str,
    error_count: usize,
    warning_count: usize,
    colorize: bool,
) -> std::io::Result<()> {
    write!(writer, "{path_label}: ")?;
    let error_word = if error_count == 1 { "error" } else { "errors" };
    let warning_word = if warning_count == 1 {
        "warning"
    } else {
        "warnings"
    };
    if error_count > 0 && warning_count > 0 {
        if colorize {
            writeln!(
                writer,
                "{error_count} {BOLD_RED}{error_word}{RESET}, {warning_count} {BOLD_YELLOW}{warning_word}{RESET}"
            )
        } else {
            writeln!(
                writer,
                "{error_count} {error_word}, {warning_count} {warning_word}"
            )
        }
    } else if error_count > 0 {
        if colorize {
            writeln!(writer, "{error_count} {BOLD_RED}{error_word}{RESET}")
        } else {
            writeln!(writer, "{error_count} {error_word}")
        }
    } else if warning_count > 0 {
        if colorize {
            writeln!(writer, "{warning_count} {BOLD_YELLOW}{warning_word}{RESET}")
        } else {
            writeln!(writer, "{warning_count} {warning_word}")
        }
    } else {
        writeln!(writer, "0 issues")
    }
}

fn render_truncated_footer(
    writer: &mut impl Write,
    path_label: &str,
    colorize: bool,
) -> std::io::Result<()> {
    if !colorize {
        writeln!(
            writer,
            "lint diagnostics truncated (use 'kost lint {path_label}' to view full report)"
        )
    } else {
        writeln!(
            writer,
            "{CYAN}lint diagnostics truncated (use 'kost lint {path_label}' to view full report){RESET}"
        )
    }
}

fn render_collapsed_footer(
    writer: &mut impl Write,
    violated_count: usize,
    colorize: bool,
) -> std::io::Result<()> {
    let file_word = if violated_count == 1 { "file" } else { "files" };
    if !colorize {
        writeln!(
            writer,
            "lint collapsed for {violated_count} {file_word} (use 'kost lint <filepath>' to view full report)"
        )
    } else {
        writeln!(
            writer,
            "{CYAN}lint collapsed for {violated_count} {file_word} (use 'kost lint <filepath>' to view full report){RESET}"
        )
    }
}

/// Renders a single diagnostic with optional path prefix and ANSI colorized severity keyword.
pub fn render_diagnostic<W: Write>(
    writer: &mut W,
    diagnostic: &Diagnostic,
    path: Option<&str>,
    colorize: bool,
) -> std::io::Result<()> {
    if colorize {
        if let Some(file_path) = path {
            write!(writer, "{file_path}{DIM}:{RESET}")?;
        }
        write!(
            writer,
            "{YELLOW}{}{RESET}{DIM}:{RESET}{YELLOW}{}{RESET}{DIM}:{RESET} ",
            diagnostic.range.start_line, diagnostic.range.start_col,
        )?;
        let sev_color = diagnostic.severity.color().unwrap_or(RESET);
        writeln!(
            writer,
            "{BOLD}[{}] {} ({RESET}{sev_color}{}{RESET}{BOLD}){RESET}",
            diagnostic.rule_id.as_str(),
            diagnostic.message.as_str(),
            diagnostic.severity.as_str(),
        )
    } else {
        if let Some(file_path) = path {
            write!(writer, "{file_path}:")?;
        }
        writeln!(
            writer,
            "{}:{}: [{}] {} ({})",
            diagnostic.range.start_line,
            diagnostic.range.start_col,
            diagnostic.rule_id.as_str(),
            diagnostic.message.as_str(),
            diagnostic.severity.as_str(),
        )
    }
}
