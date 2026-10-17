//! Built-in AST rule enforcing spacing around double colon operator (Ktlint rule 18).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces no whitespace around the double colon callable/class reference operator.
pub struct DoubleColonSpacing;

impl AstRule for DoubleColonSpacing {
    fn id(&self) -> &'static str {
        "double-colon-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["::"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if ctx.prev_sibling().is_some()
            && let WhitespaceBefore::Spaces(n) = spaces_before(node, source)
            && n > 0
        {
            ctx.report(node, "Unexpected spacing before '::'");
        }

        if let WhitespaceAfter::Spaces(n) = spaces_after(node, source)
            && n > 0
        {
            ctx.report(node, "Unexpected spacing after '::'");
        }
    }
}
