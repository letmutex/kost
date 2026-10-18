//! Built-in AST rule enforcing KDoc line wrapping (Ktlint rule 79).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces KDoc comments start and end on dedicated lines.
pub struct KdocWrapping;

impl AstRule for KdocWrapping {
    fn id(&self) -> &'static str {
        "kdoc-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if !is_kdoc(text) {
            return;
        }

        check_kdoc_wrapping(node, text, source, ctx);
    }
}

fn is_kdoc(text: &str) -> bool {
    text.starts_with("/**") && !text.starts_with("/***") && text.ends_with("*/") && text != "/**/"
}

fn check_kdoc_wrapping(node: tree_sitter::Node, text: &str, source: &[u8], ctx: &mut LintCtx) {
    let message =
        "A KDoc comment should start and end on a line that does not contain any other element";

    if spaces_before(node, source) != WhitespaceBefore::StartOfLine {
        ctx.report(node, message);
        return;
    }

    if node.start_position().row == node.end_position().row {
        if !matches!(
            spaces_after(node, source),
            WhitespaceAfter::EndOfLine { .. }
        ) {
            let end_line = (node.end_position().row + 1) as u32;
            ctx.report_range(
                crate::types::TextRange::new(end_line, 1, end_line, 1),
                message,
            );
        }
        return;
    }

    let Some(first_newline) = text.find('\n') else {
        return;
    };
    if !text[3..first_newline].trim().is_empty() {
        ctx.report(node, message);
    }

    let Some(last_newline) = text.rfind('\n') else {
        return;
    };
    let closing_slice = &text[last_newline + 1..];
    let end_line = (node.end_position().row + 1) as u32;

    if let Some(content_before_closing) = closing_slice.strip_suffix("*/")
        && !content_before_closing.trim().is_empty()
    {
        ctx.report_range(
            crate::types::TextRange::new(end_line, 1, end_line, 1),
            message,
        );
        return;
    }

    if !matches!(
        spaces_after(node, source),
        WhitespaceAfter::EndOfLine { .. }
    ) {
        ctx.report_range(
            crate::types::TextRange::new(end_line, 1, end_line, 1),
            message,
        );
    }
}
