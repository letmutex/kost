//! Built-in AST rule enforcing spacing around range operators (Ktlint rule 22).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces zero spaces around range operators `..` and `..<`.
pub struct RangeSpacing;

impl AstRule for RangeSpacing {
    fn id(&self) -> &'static str {
        "range-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["..", "..<"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let operator = node.kind();

        match spaces_before(node, source) {
            WhitespaceBefore::Spaces(count) if count > 0 => {
                let message = common::concat_str!("Unexpected spacing before '", operator, "'");
                ctx.report(node, message);
            }
            WhitespaceBefore::StartOfLine => {
                let message = common::concat_str!("Unexpected spacing before '", operator, "'");
                ctx.report(node, message);
            }
            _ => {}
        }

        match spaces_after(node, source) {
            WhitespaceAfter::Spaces(count) if count > 0 => {
                let message = common::concat_str!("Unexpected spacing after '", operator, "'");
                ctx.report(node, message);
            }
            WhitespaceAfter::EndOfLine { .. } => {
                let message = common::concat_str!("Unexpected spacing after '", operator, "'");
                ctx.report(node, message);
            }
            _ => {}
        }
    }
}
