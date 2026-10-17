//! Built-in AST rule enforcing wrapping of block comments (Ktlint rule 93).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, is_kdoc, spaces_after, spaces_before};

/// Rule that enforces external wrapping of block comments.
pub struct CommentWrapping;

impl AstRule for CommentWrapping {
    fn id(&self) -> &'static str {
        "comment-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if !text.starts_with("/*") || !text.ends_with("*/") || is_kdoc(text) {
            return;
        }

        let has_code_before = spaces_before(node, source) != WhitespaceBefore::StartOfLine;
        let has_code_after = !matches!(
            spaces_after(node, source),
            WhitespaceAfter::EndOfLine { .. }
        );

        let is_single_line = node.start_position().row == node.end_position().row;

        if has_code_before && has_code_after {
            if is_enclosed_in_single_line_braces(node) {
                return;
            }
            if is_single_line {
                ctx.report(
                    node,
                    "A block comment in between other elements on the same line is disallowed",
                );
            } else {
                ctx.report(
                    node,
                    "A block comment starting on same line as another element and ending on another line before another element is disallowed",
                );
            }
            return;
        }

        if has_code_before {
            if !is_single_line {
                ctx.report(
                    node,
                    "A block comment after any other element on the same line must be separated by a new line",
                );
            }
            return;
        }

        if has_code_after {
            ctx.report(
                node,
                "A block comment may not be followed by any other element on that same line",
            );
        }
    }
}

fn is_enclosed_in_single_line_braces(node: tree_sitter::Node) -> bool {
    let Some(prev) = node.prev_sibling() else {
        return false;
    };
    let Some(next) = node.next_sibling() else {
        return false;
    };
    prev.kind() == "{" && next.kind() == "}" && prev.start_position().row == next.end_position().row
}
