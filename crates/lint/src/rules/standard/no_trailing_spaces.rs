//! Built-in AST rule forbidding trailing whitespace (Ktlint rule 41).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

/// Rule that detects trailing whitespace at the end of lines.
pub struct NoTrailingSpaces;

impl AstRule for NoTrailingSpaces {
    fn id(&self) -> &'static str {
        "no-trailing-spaces"
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
        if !ctx.trim_trailing_whitespace() {
            return;
        }
        let ranges = std::mem::take(&mut ctx.multiline_string_ranges);
        scan_trailing_spaces(source, &ranges, ctx);
        ctx.multiline_string_ranges = ranges;
    }
}

fn is_in_excluded_range(offset: usize, excluded: &[(usize, usize)]) -> bool {
    match excluded.binary_search_by_key(&offset, |&(start, _)| start) {
        Ok(_) => true,
        Err(index) => index > 0 && offset < excluded[index - 1].1,
    }
}

fn scan_trailing_spaces(source: &[u8], excluded: &[(usize, usize)], ctx: &mut LintCtx) {
    let mut line_number: u32 = 1;
    let mut line_start_byte: usize = 0;
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

        if content_end > line_start_byte {
            let mut trailing_start = content_end;
            while trailing_start > line_start_byte
                && (source[trailing_start - 1] == b' ' || source[trailing_start - 1] == b'\t')
            {
                trailing_start -= 1;
            }

            if trailing_start < content_end && !is_in_excluded_range(trailing_start, excluded) {
                let start_column = (trailing_start - line_start_byte + 1) as u32;
                let end_column = (content_end - line_start_byte + 1) as u32;
                ctx.report_range(
                    TextRange {
                        start_line: line_number,
                        start_col: start_column,
                        end_line: line_number,
                        end_col: end_column,
                    },
                    "Trailing space(s)",
                );
            }
        }

        line_start_byte = if line_end_byte < total_len {
            line_end_byte + 1
        } else {
            total_len
        };
        line_number += 1;
    }
}
