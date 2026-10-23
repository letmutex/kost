//! Unified diff generation for formatting diagnostics.

use common::color::{CYAN, GREEN, RED, RESET};
use similar::{ChangeTag, TextDiff};
use std::fmt::Write;

struct DiffMetadata {
    deletions: usize,
    insertions: usize,
    gutter_width: usize,
    eof_line: usize,
    eof_newline_added: bool,
    eof_newline_removed: bool,
}

/// Generates a unified diff comparing `original` and `formatted`.
///
/// Returns `None` if `original == formatted`.
pub fn create_diff(
    original: &str,
    formatted: &str,
    file_label: &str,
    colorize: bool,
) -> Option<String> {
    if original == formatted {
        return None;
    }

    let diff = TextDiff::from_lines(original, formatted);
    let mut unified = diff.unified_diff();
    unified.context_radius(3).header(file_label, file_label);

    let hunks: Vec<_> = unified.iter_hunks().collect();
    if hunks.is_empty() {
        return None;
    }

    let metadata = inspect_diff_metadata(original, formatted, &hunks);
    let mut result = String::with_capacity(256);

    render_header(
        &mut result,
        file_label,
        metadata.deletions,
        metadata.insertions,
        colorize,
    );

    let hunks_len = hunks.len();
    for (hunk_idx, hunk) in hunks.iter().enumerate() {
        render_hunk_header(&mut result, hunk, colorize);

        let mut in_insert = false;
        for change in hunk.iter_changes() {
            let is_insert = change.tag() == ChangeTag::Insert;
            let show_line_num = !is_insert || !in_insert;
            in_insert = is_insert;
            render_change(
                &mut result,
                &change,
                metadata.gutter_width,
                show_line_num,
                colorize,
            );
        }

        if hunk_idx == hunks_len - 1 {
            render_eof_marker(
                &mut result,
                metadata.eof_line,
                metadata.gutter_width,
                metadata.eof_newline_added,
                metadata.eof_newline_removed,
                colorize,
            );
        }
    }

    Some(result)
}

fn inspect_diff_metadata(
    original: &str,
    formatted: &str,
    hunks: &[similar::udiff::UnifiedDiffHunk<'_, '_, '_, str>],
) -> DiffMetadata {
    let eof_newline_added = !original.ends_with('\n') && formatted.ends_with('\n');
    let eof_newline_removed = original.ends_with('\n') && !formatted.ends_with('\n');
    let eof_line = if eof_newline_added {
        formatted.split_inclusive('\n').count() + 1
    } else if eof_newline_removed {
        original.split_inclusive('\n').count() + 1
    } else {
        0
    };

    let mut deletions = usize::from(eof_newline_removed);
    let mut insertions = usize::from(eof_newline_added);
    let mut max_line = if eof_newline_added || eof_newline_removed {
        eof_line
    } else {
        0
    };

    for hunk in hunks {
        let mut in_insert = false;
        for change in hunk.iter_changes() {
            if let Some(index) = change.old_index() {
                max_line = max_line.max(index + 1);
            }
            let is_insert = change.tag() == ChangeTag::Insert;
            if is_insert
                && !in_insert
                && let Some(index) = change.new_index()
            {
                max_line = max_line.max(index + 1);
            }
            in_insert = is_insert;
            match change.tag() {
                ChangeTag::Delete => deletions += 1,
                ChangeTag::Insert => insertions += 1,
                ChangeTag::Equal => {}
            }
        }
    }

    let gutter_width = if max_line == 0 {
        1
    } else {
        max_line.ilog10() as usize + 1
    };

    DiffMetadata {
        deletions,
        insertions,
        gutter_width,
        eof_line,
        eof_newline_added,
        eof_newline_removed,
    }
}

fn render_hunk_header(
    result: &mut String,
    hunk: &similar::udiff::UnifiedDiffHunk<'_, '_, '_, str>,
    colorize: bool,
) {
    if !colorize {
        let _ = writeln!(result, "{}", hunk.header());
    } else {
        let _ = writeln!(result, "{CYAN}{}{RESET}", hunk.header());
    }
}

fn render_header(
    result: &mut String,
    file_label: &str,
    deletions: usize,
    insertions: usize,
    colorize: bool,
) {
    let _ = write!(result, "{file_label}");
    if deletions > 0 {
        if colorize {
            let _ = write!(result, " {RED}-{RESET}{deletions}");
        } else {
            let _ = write!(result, " -{deletions}");
        }
    }
    if insertions > 0 {
        if colorize {
            let _ = write!(result, " {GREEN}+{RESET}{insertions}");
        } else {
            let _ = write!(result, " +{insertions}");
        }
    }
    result.push('\n');
}

fn render_change(
    result: &mut String,
    change: &similar::Change<&str>,
    width: usize,
    show_line_num: bool,
    colorize: bool,
) {
    let line = change.value();
    let (content, newline) = if let Some(stripped) = line.strip_suffix("\r\n") {
        (stripped, "\r\n")
    } else if let Some(stripped) = line.strip_suffix('\n') {
        (stripped, "\n")
    } else {
        (line, "\n")
    };

    let sign = match change.tag() {
        ChangeTag::Equal => ' ',
        ChangeTag::Delete => '-',
        ChangeTag::Insert => '+',
    };

    if !colorize {
        if show_line_num {
            let line_num = match change.tag() {
                ChangeTag::Equal | ChangeTag::Delete => change.old_index().unwrap() + 1,
                ChangeTag::Insert => change.new_index().unwrap() + 1,
            };
            let _ = write!(result, "{line_num:>width$} {sign}{content}{newline}");
        } else {
            let _ = write!(result, "{:>width$} {sign}{content}{newline}", "");
        }
    } else {
        match change.tag() {
            ChangeTag::Equal => {
                let line_num = change.old_index().unwrap() + 1;
                let _ = write!(result, "{line_num:>width$}  {content}{newline}");
            }
            ChangeTag::Delete => {
                let line_num = change.old_index().unwrap() + 1;
                let _ = write!(result, "{line_num:>width$} {RED}-{content}{RESET}{newline}");
            }
            ChangeTag::Insert => {
                if show_line_num {
                    let line_num = change.new_index().unwrap() + 1;
                    let _ = write!(
                        result,
                        "{line_num:>width$} {GREEN}+{content}{RESET}{newline}"
                    );
                } else {
                    let _ = write!(result, "{:>width$} {GREEN}+{content}{RESET}{newline}", "");
                }
            }
        }
    }
}

fn render_eof_marker(
    result: &mut String,
    eof_line: usize,
    width: usize,
    eof_newline_added: bool,
    eof_newline_removed: bool,
    colorize: bool,
) {
    if eof_newline_added {
        if !colorize {
            let _ = writeln!(result, "{eof_line:>width$} +");
        } else {
            let _ = writeln!(result, "{eof_line:>width$} {GREEN}+{RESET}");
        }
    } else if eof_newline_removed {
        if !colorize {
            let _ = writeln!(result, "{eof_line:>width$} -");
        } else {
            let _ = writeln!(result, "{eof_line:>width$} {RED}-{RESET}");
        }
    }
}
