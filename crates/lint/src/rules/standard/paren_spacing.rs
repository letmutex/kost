//! Built-in AST rule enforcing spacing around parentheses (Ktlint rule 21).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around parentheses.
pub struct ParenSpacing;

impl AstRule for ParenSpacing {
    fn id(&self) -> &'static str {
        "paren-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["(", ")"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "(" {
            check_opening_paren(node, source, ctx);
        } else {
            check_closing_paren(node, source, ctx);
        }
    }
}

fn check_opening_paren(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    match spaces_after(node, source) {
        WhitespaceAfter::Spaces(count) if count > 0 => {
            ctx.report(node, "Unexpected spacing after '('");
        }
        WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
            ctx.report(node, "Unexpected spacing after '('");
        }
        _ => {}
    }
}

fn check_closing_paren(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if let WhitespaceBefore::Spaces(count) = spaces_before(node, source)
        && count > 0
    {
        if let Some(prev) = node.prev_sibling()
            && prev.kind() == "("
        {
            return;
        }
        ctx.report(node, "Unexpected spacing before ')'");
    }
}
