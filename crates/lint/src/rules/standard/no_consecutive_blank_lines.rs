//! Built-in AST rule forbidding consecutive blank lines (Ktlint rule 70).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

/// Rule that limits consecutive blank lines throughout the file to at most one.
pub struct NoConsecutiveBlankLines;

impl AstRule for NoConsecutiveBlankLines {
    fn id(&self) -> &'static str {
        "no-consecutive-blank-lines"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn needs_file_exclusions(&self) -> bool {
        true
    }

    fn check_node(&self, _node: tree_sitter::Node, _source: &[u8], _ctx: &mut LintCtx) {}

    fn after_node(&self, _node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let ranges = std::mem::take(&mut ctx.multiline_string_and_comment_ranges);
        scan_consecutive_blank_lines(source, &ranges, ctx);
        ctx.multiline_string_and_comment_ranges = ranges;
    }
}

fn is_in_excluded_range(offset: usize, excluded: &[(usize, usize)]) -> bool {
    match excluded.binary_search_by_key(&offset, |&(start, _)| start) {
        Ok(_) => true,
        Err(index) => index > 0 && offset < excluded[index - 1].1,
    }
}

fn scan_consecutive_blank_lines(source: &[u8], excluded: &[(usize, usize)], ctx: &mut LintCtx) {
    let mut line_number: u32 = 1;
    let mut line_start_byte: usize = 0;
    let mut consecutive_blank_lines: usize = 0;
    let total_len = source.len();

    while line_start_byte < total_len {
        let mut line_end_byte = line_start_byte;
        while line_end_byte < total_len && source[line_end_byte] != b'\n' {
            line_end_byte += 1;
        }

        let mut content_end = line_end_byte;
        if content_end > line_start_byte && source[content_end - 1] == b'\r' {
            content_end -= 1;
        }

        let mut is_blank = true;
        for &byte in &source[line_start_byte..content_end] {
            if byte != b' ' && byte != b'\t' {
                is_blank = false;
                break;
            }
        }

        if is_in_excluded_range(line_start_byte, excluded) {
            consecutive_blank_lines = 0;
        } else if is_blank {
            consecutive_blank_lines += 1;
            if consecutive_blank_lines >= 2 {
                ctx.report_range(
                    TextRange {
                        start_line: line_number,
                        start_col: 1,
                        end_line: line_number,
                        end_col: 1,
                    },
                    "Needless blank line(s)",
                );
            }
        } else {
            consecutive_blank_lines = 0;
        }

        line_start_byte = if line_end_byte < total_len {
            line_end_byte + 1
        } else {
            total_len
        };
        line_number += 1;
    }
}
