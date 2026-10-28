//! Built-in AST rule enforcing indentation conventions (Ktlint rule 102).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

/// Rule enforcing uniform 4-space indentation.
pub struct Indent;

impl AstRule for Indent {
    fn id(&self) -> &'static str {
        "indent"
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
        if source.is_empty() {
            return;
        }

        if source.starts_with(b" ") || source.starts_with(b"\t") {
            let mut length = 0;
            while length < source.len() && (source[length] == b' ' || source[length] == b'\t') {
                length += 1;
            }
            if length < source.len() && source[length] != b'\n' && source[length] != b'\r' {
                ctx.report_range(
                    TextRange::new(1, 1, 1, (length + 1) as u32),
                    "Unexpected indentation",
                );
            }
        }

        let ranges = std::mem::take(&mut ctx.multiline_string_ranges);
        scan_line_indentation(source, &ranges, ctx);
        ctx.multiline_string_ranges = ranges;
    }
}

fn is_in_excluded_range(offset: usize, excluded: &[(usize, usize)]) -> bool {
    match excluded.binary_search_by_key(&offset, |&(start, _)| start) {
        Ok(_) => true,
        Err(index) => index > 0 && offset < excluded[index - 1].1,
    }
}

fn scan_line_indentation(source: &[u8], excluded: &[(usize, usize)], ctx: &mut LintCtx) {
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

        if !is_in_excluded_range(line_start, excluded) {
            let mut indent_len = 0;
            let mut has_tab = false;
            while line_start + indent_len < actual_end
                && (source[line_start + indent_len] == b' '
                    || source[line_start + indent_len] == b'\t')
            {
                if source[line_start + indent_len] == b'\t' {
                    has_tab = true;
                }
                indent_len += 1;
            }

            let line_content = &source[line_start + indent_len..actual_end];
            if !line_content.is_empty()
                && has_tab
                && ctx.indent_style() != common::editorconfig::IndentStyle::Tab
            {
                ctx.report_range(
                    TextRange::new(line_number, 1, line_number, (indent_len + 1) as u32),
                    "Unexpected tab character(s)",
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
