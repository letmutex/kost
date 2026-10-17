//! Smart clean diff presentation across multiple files.

use cli::PrintMode;
use common::color::{CYAN, RESET};
use std::io::Write;

/// Display tier for smart diff formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayTier {
    Full,
    Truncated,
    Collapsed,
}

/// Resolves the effective display tier given user mode and number of unformatted files.
pub fn resolve_display_tier(mode: PrintMode, changed_count: usize) -> DisplayTier {
    match mode {
        PrintMode::Auto => match changed_count {
            0..=10 => DisplayTier::Full,
            11..=20 => DisplayTier::Truncated,
            _ => DisplayTier::Collapsed,
        },
        PrintMode::Full => DisplayTier::Full,
        PrintMode::Truncated => DisplayTier::Truncated,
        PrintMode::Collapsed => DisplayTier::Collapsed,
    }
}

/// Truncates a diff string after at most `max_lines` newlines, returning a slice and remaining line count.
pub fn truncate_diff_to_lines(diff: &str, max_lines: usize) -> Option<(&str, usize)> {
    if max_lines == 0 {
        let remaining = diff.lines().count();
        return if remaining > 0 {
            Some(("", remaining))
        } else {
            None
        };
    }
    let mut count = 0;
    for (idx, ch) in diff.char_indices() {
        if ch == '\n' {
            count += 1;
            if count == max_lines {
                let remaining = diff[idx + 1..].lines().count();
                if remaining > 0 {
                    return Some((&diff[..=idx], remaining));
                }
                return None;
            }
        }
    }
    None
}

/// Renders smart diffs for all unformatted files according to the active display tier.
pub fn render_smart_diffs<W: Write>(
    writer: &mut W,
    tier: DisplayTier,
    unformatted_files: &[(&str, Option<&str>)],
    colorize: bool,
) -> std::io::Result<()> {
    match tier {
        DisplayTier::Full => render_full_diffs(writer, unformatted_files),
        DisplayTier::Truncated => render_truncated_diffs(writer, unformatted_files, colorize),
        DisplayTier::Collapsed => render_collapsed_diffs(writer, unformatted_files, colorize),
    }
}

fn render_full_diffs<W: Write>(
    writer: &mut W,
    unformatted_files: &[(&str, Option<&str>)],
) -> std::io::Result<()> {
    for (path_label, diff) in unformatted_files {
        if let Some(diff_text) = diff {
            write!(writer, "{diff_text}")?;
        } else {
            writeln!(writer, "---+++ {path_label}")?;
        }
    }
    Ok(())
}

fn render_truncated_diffs<W: Write>(
    writer: &mut W,
    unformatted_files: &[(&str, Option<&str>)],
    colorize: bool,
) -> std::io::Result<()> {
    for (path_label, diff) in unformatted_files {
        if let Some(diff_text) = diff {
            if let Some((truncated, _remaining)) = truncate_diff_to_lines(diff_text, 20) {
                write!(writer, "{truncated}")?;
                render_truncated_footer(writer, path_label, colorize)?;
            } else {
                write!(writer, "{diff_text}")?;
            }
        } else {
            writeln!(writer, "---+++ {path_label}")?;
        }
    }
    Ok(())
}

fn render_collapsed_diffs<W: Write>(
    writer: &mut W,
    unformatted_files: &[(&str, Option<&str>)],
    colorize: bool,
) -> std::io::Result<()> {
    for (path_label, diff) in unformatted_files {
        if let Some(header) = diff.and_then(|text| text.lines().next()) {
            writeln!(writer, "{header}")?;
        } else {
            writeln!(writer, "---+++ {path_label}")?;
        }
    }
    render_collapsed_footer(writer, unformatted_files.len(), colorize)?;
    Ok(())
}

fn render_truncated_footer(
    writer: &mut impl Write,
    path_label: &str,
    colorize: bool,
) -> std::io::Result<()> {
    if !colorize {
        writeln!(
            writer,
            "diff truncated (use 'kost fmt --check {path_label}' to view full diff)"
        )
    } else {
        writeln!(
            writer,
            "{CYAN}diff truncated (use 'kost fmt --check {path_label}' to view full diff){RESET}"
        )
    }
}

fn render_collapsed_footer(
    writer: &mut impl Write,
    changed_count: usize,
    colorize: bool,
) -> std::io::Result<()> {
    let file_word = if changed_count == 1 { "file" } else { "files" };
    if !colorize {
        writeln!(
            writer,
            "diff collapsed for {changed_count} {file_word} (use 'kost fmt --check <filepath>' to view full diff)"
        )
    } else {
        writeln!(
            writer,
            "{CYAN}diff collapsed for {changed_count} {file_word} (use 'kost fmt --check <filepath>' to view full diff){RESET}"
        )
    }
}
