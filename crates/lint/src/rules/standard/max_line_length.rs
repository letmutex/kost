//! Built-in AST rule enforcing maximum line length (Ktlint rule 103).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

const DEFAULT_MAX_LINE_LENGTH: usize = 140;

/// Rule that detects lines exceeding the configured maximum character limit.
pub struct MaxLineLength;

impl AstRule for MaxLineLength {
    fn id(&self) -> &'static str {
        "max-line-length"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn needs_file_exclusions(&self) -> bool {
        true
    }

    fn check_node(&self, _node: tree_sitter::Node, _source: &[u8], _ctx: &mut LintCtx) {}

    fn has_after_node(&self) -> bool {
        true
    }

    fn after_node(&self, _node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let max_len = match ctx.max_line_length() {
            Some(common::editorconfig::LineLengthSetting::Off) => return,
            Some(common::editorconfig::LineLengthSetting::Value(len)) => len,
            None => DEFAULT_MAX_LINE_LENGTH,
        };
        let ranges = std::mem::take(&mut ctx.max_line_length_exclusions);
        scan_line_lengths(source, &ranges, max_len, ctx);
        ctx.max_line_length_exclusions = ranges;
    }
}

fn is_in_excluded_range(offset: usize, excluded: &[(usize, usize)]) -> bool {
    match excluded.binary_search_by_key(&offset, |&(start, _)| start) {
        Ok(_) => true,
        Err(index) => index > 0 && offset < excluded[index - 1].1,
    }
}

fn scan_line_lengths(
    source: &[u8],
    excluded: &[(usize, usize)],
    max_len: usize,
    ctx: &mut LintCtx,
) {
    let mut line_number: u32 = 1;
    let mut line_start: usize = 0;
    let total_len = source.len();

    while line_start < total_len {
        let mut line_end = line_start;
        while line_end < total_len && source[line_end] != b'\n' {
            line_end += 1;
        }

        let mut actual_end = line_end;
        if actual_end > line_start && source[actual_end - 1] == b'\r' {
            actual_end -= 1;
        }

        let line_bytes = &source[line_start..actual_end];
        let line_char_len = std::str::from_utf8(line_bytes)
            .map_or_else(|_| line_bytes.len(), |s| s.chars().count());

        if line_char_len > max_len && !is_in_excluded_range(line_start, excluded) {
            let trimmed = trim_ascii_start(line_bytes);

            if !is_exempt_line(trimmed) {
                let violation_col = (max_len + 1) as u32;
                let message = if max_len == DEFAULT_MAX_LINE_LENGTH {
                    "Exceeded max line length (140)".to_string()
                } else {
                    common::concat_str!("Exceeded max line length (", &max_len.to_string(), ")")
                };
                ctx.report_range(
                    TextRange::new(
                        line_number,
                        violation_col,
                        line_number,
                        (line_char_len + 1) as u32,
                    ),
                    message,
                );
            }
        }

        if line_end < total_len && source[line_end] == b'\n' {
            line_start = line_end + 1;
        } else {
            line_start = line_end;
        }
        line_number += 1;
    }
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && (bytes[start] == b' ' || bytes[start] == b'\t') {
        start += 1;
    }
    &bytes[start..]
}

fn is_exempt_line(trimmed: &[u8]) -> bool {
    trimmed.starts_with(b"package ")
        || trimmed.starts_with(b"import ")
        || trimmed.starts_with(b"//")
        || (trimmed.starts_with(b"/*") && trimmed.ends_with(b"*/"))
}
