//! Built-in AST rule forbidding unnecessary consecutive spaces (Ktlint rule 40).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

/// Rule that detects multiple consecutive spaces outside of indentation and comments/strings.
pub struct NoMultiSpaces;

impl AstRule for NoMultiSpaces {
    fn id(&self) -> &'static str {
        "no-multi-spaces"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn needs_file_exclusions(&self) -> bool {
        true
    }

    fn check_node(&self, _node: tree_sitter::Node, _source: &[u8], _ctx: &mut LintCtx) {}

    fn after_node(&self, _node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let ranges = std::mem::take(&mut ctx.all_string_and_comment_ranges);
        scan_multi_spaces(source, &ranges, ctx);
        ctx.all_string_and_comment_ranges = ranges;
    }
}

fn is_excluded(offset: usize, excluded: &[(usize, usize)]) -> bool {
    match excluded.binary_search_by_key(&offset, |&(start, _)| start) {
        Ok(_) => true,
        Err(idx) => idx > 0 && offset < excluded[idx - 1].1,
    }
}

fn scan_multi_spaces(source: &[u8], excluded: &[(usize, usize)], ctx: &mut LintCtx) {
    let mut line_num: u32 = 1;
    let mut line_start_byte: usize = 0;
    let len = source.len();

    while line_start_byte < len {
        let mut line_end_byte = line_start_byte;
        while line_end_byte < len && source[line_end_byte] != b'\n' {
            line_end_byte += 1;
        }

        let mut content_end = line_end_byte;
        if content_end > line_start_byte && source[content_end - 1] == b'\r' {
            content_end -= 1;
        }

        let line_bytes = &source[line_start_byte..content_end];
        let mut first_non_space = 0;
        while first_non_space < line_bytes.len()
            && (line_bytes[first_non_space] == b' ' || line_bytes[first_non_space] == b'\t')
        {
            first_non_space += 1;
        }

        if first_non_space < line_bytes.len() {
            let mut i = first_non_space;
            while i < line_bytes.len() {
                if line_bytes[i] == b' ' {
                    let run_start = i;
                    while i < line_bytes.len() && line_bytes[i] == b' ' {
                        i += 1;
                    }
                    let run_len = i - run_start;
                    if run_len > 1 {
                        let global_offset = line_start_byte + run_start;
                        if !is_excluded(global_offset, excluded) {
                            let start_col = (run_start + 1) as u32;
                            let end_col = start_col + run_len as u32;
                            ctx.report_range(
                                TextRange {
                                    start_line: line_num,
                                    start_col,
                                    end_line: line_num,
                                    end_col,
                                },
                                "Unnecessary space(s)",
                            );
                        }
                    }
                } else {
                    i += 1;
                }
            }
        }

        line_start_byte = if line_end_byte < len {
            line_end_byte + 1
        } else {
            len
        };
        line_num += 1;
    }
}
