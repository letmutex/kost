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

        if !is_properly_wrapped_kdoc(node, text, source) {
            ctx.report(
                node,
                "A KDoc comment should start and end on a line that does not contain any other element",
            );
        }
    }
}

fn is_kdoc(text: &str) -> bool {
    text.starts_with("/**") && !text.starts_with("/***") && text.ends_with("*/") && text != "/**/"
}

fn is_properly_wrapped_kdoc(node: tree_sitter::Node, text: &str, source: &[u8]) -> bool {
    if spaces_before(node, source) != WhitespaceBefore::StartOfLine {
        return false;
    }

    if !matches!(
        spaces_after(node, source),
        WhitespaceAfter::EndOfLine { .. }
    ) {
        return false;
    }

    if node.start_position().row == node.end_position().row {
        return false;
    }

    let Some(first_newline) = text.find('\n') else {
        return false;
    };
    if !text[3..first_newline].trim().is_empty() {
        return false;
    }

    let Some(last_newline) = text.rfind('\n') else {
        return false;
    };
    let closing_slice = &text[last_newline + 1..];
    if closing_slice.len() < 2 {
        return false;
    }

    let content_before_closing = &closing_slice[..closing_slice.len() - 2];
    content_before_closing.trim().is_empty()
}
