//! Full snippet formatting and rendering for lint diagnostics.

use crate::smart_lint::render_diagnostic;
use crate::types::{Diagnostic, Severity};
use common::color::{BOLD, DIM, RESET, YELLOW};
use std::io::Write;

/// Renders a diagnostic in full snippet mode with context lines, dimmed gutter, caret, and stack location.
pub fn render_full_snippet_diagnostic<W: Write>(
    writer: &mut W,
    diagnostic: &Diagnostic,
    lines: &[&str],
    path: Option<&str>,
    colorize: bool,
) -> std::io::Result<()> {
    if diagnostic.range.start_line == 0 {
        return render_diagnostic(writer, diagnostic, path, colorize);
    }

    let target_idx = (diagnostic.range.start_line as usize).saturating_sub(1);
    if target_idx >= lines.len() {
        return render_diagnostic(writer, diagnostic, path, colorize);
    }

    // Up to 2 extra lines before the diagnosis line, trimming consecutive leading empty lines.
    let mut start_idx = target_idx.saturating_sub(2);
    while start_idx < target_idx && lines[start_idx].trim().is_empty() {
        start_idx += 1;
    }

    let max_line = diagnostic.range.start_line;
    let gutter_width = if max_line == 0 {
        1
    } else {
        max_line.ilog10() as usize + 1
    };

    render_context_lines(writer, lines, start_idx, target_idx, gutter_width, colorize)?;
    render_caret_line(
        writer,
        lines[target_idx],
        diagnostic.range.start_col,
        gutter_width,
        diagnostic.severity,
        colorize,
    )?;
    render_diagnostic_footer(writer, diagnostic, path, colorize)
}

fn render_context_lines<W: Write>(
    writer: &mut W,
    lines: &[&str],
    start_idx: usize,
    target_idx: usize,
    gutter_width: usize,
    colorize: bool,
) -> std::io::Result<()> {
    for (offset, content) in lines[start_idx..=target_idx].iter().enumerate() {
        let current_idx = start_idx + offset;
        let line_num = current_idx + 1;
        let is_diagnosis_line = current_idx == target_idx;
        let dim_gutter = colorize && !is_diagnosis_line;

        if dim_gutter {
            if content.is_empty() {
                writeln!(writer, "{DIM}{line_num:>gutter_width$} |{RESET}")?;
            } else {
                writeln!(writer, "{DIM}{line_num:>gutter_width$} | {RESET}{content}")?;
            }
        } else if content.is_empty() {
            writeln!(writer, "{line_num:>gutter_width$} |")?;
        } else {
            writeln!(writer, "{line_num:>gutter_width$} | {content}")?;
        }
    }
    Ok(())
}

fn render_caret_line<W: Write>(
    writer: &mut W,
    target_line: &str,
    start_col: u32,
    gutter_width: usize,
    severity: Severity,
    colorize: bool,
) -> std::io::Result<()> {
    let col_offset = (start_col as usize).saturating_sub(1);
    let mut indent = String::with_capacity(gutter_width + 3 + col_offset);
    for _ in 0..(gutter_width + 3) {
        indent.push(' ');
    }
    for (byte_idx, ch) in target_line.char_indices() {
        if byte_idx >= col_offset {
            break;
        }
        if ch == '\t' {
            indent.push('\t');
        } else {
            indent.push(' ');
        }
    }
    if col_offset > target_line.len() {
        for _ in 0..(col_offset - target_line.len()) {
            indent.push(' ');
        }
    }

    if colorize {
        let caret_color = severity.color().unwrap_or(RESET);
        writeln!(writer, "{indent}{caret_color}^{RESET}")
    } else {
        writeln!(writer, "{indent}^")
    }
}

fn render_diagnostic_footer<W: Write>(
    writer: &mut W,
    diagnostic: &Diagnostic,
    path: Option<&str>,
    colorize: bool,
) -> std::io::Result<()> {
    if colorize {
        let sev_color = diagnostic.severity.color().unwrap_or(RESET);
        writeln!(
            writer,
            "{sev_color}{}{RESET}{BOLD}: {}: {}{RESET}",
            diagnostic.severity.as_str(),
            diagnostic.rule_id.as_str(),
            diagnostic.message.as_str()
        )?;
        let path_label = path.unwrap_or("<input>");
        writeln!(
            writer,
            "    {DIM}at{RESET} {path_label}{DIM}:{RESET}{YELLOW}{}{RESET}{DIM}:{RESET}{YELLOW}{}{RESET}",
            diagnostic.range.start_line, diagnostic.range.start_col
        )
    } else {
        writeln!(
            writer,
            "{}: {}: {}",
            diagnostic.severity.as_str(),
            diagnostic.rule_id.as_str(),
            diagnostic.message.as_str()
        )?;
        let path_label = path.unwrap_or("<input>");
        writeln!(
            writer,
            "    at {path_label}:{}:{}",
            diagnostic.range.start_line, diagnostic.range.start_col
        )
    }
}
