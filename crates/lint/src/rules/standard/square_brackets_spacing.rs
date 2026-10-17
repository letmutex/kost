//! Built-in AST rule enforcing spacing around square brackets (Ktlint rule 23).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around square brackets.
pub struct SquareBracketsSpacing;

impl AstRule for SquareBracketsSpacing {
    fn id(&self) -> &'static str {
        "square-brackets-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["[", "]"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "[" {
            check_opening_bracket(node, source, ctx);
        } else {
            check_closing_bracket(node, source, ctx);
        }
    }
}

fn check_opening_bracket(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if node
        .parent()
        .map(|parent| parent.kind() == "indexing_suffix")
        .unwrap_or(false)
    {
        match spaces_before(node, source) {
            WhitespaceBefore::Spaces(count) if count > 0 => {
                ctx.report(node, "Unexpected spacing before '['");
            }
            WhitespaceBefore::StartOfLine => {
                ctx.report(node, "Unexpected spacing before '['");
            }
            _ => {}
        }
    }

    match spaces_after(node, source) {
        WhitespaceAfter::Spaces(count) if count > 0 => {
            ctx.report(node, "Unexpected spacing after '['");
        }
        WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
            ctx.report(node, "Unexpected spacing after '['");
        }
        _ => {}
    }
}

fn check_closing_bracket(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if let Some(prev) = node.prev_sibling()
        && prev.kind() == "["
    {
        return;
    }

    match spaces_before(node, source) {
        WhitespaceBefore::Spaces(count) if count > 0 => {
            ctx.report(node, "Unexpected spacing before ']'");
        }
        _ => {}
    }
}
