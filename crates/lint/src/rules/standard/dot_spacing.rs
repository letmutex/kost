//! Built-in AST rule enforcing spacing around dereference and safe navigation dots (Ktlint rule 17).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces no whitespace around member access dots on a single line.
pub struct DotSpacing;

impl AstRule for DotSpacing {
    fn id(&self) -> &'static str {
        "dot-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[".", "?."]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let op = node.kind();

        if let WhitespaceBefore::Spaces(n) = spaces_before(node, source)
            && n > 0
        {
            let message = common::concat_str!("Unexpected spacing before '", op, "'");
            ctx.report(node, message);
        }

        if let WhitespaceAfter::Spaces(n) = spaces_after(node, source)
            && n > 0
        {
            let message = common::concat_str!("Unexpected spacing after '", op, "'");
            ctx.report(node, message);
        }
    }
}
