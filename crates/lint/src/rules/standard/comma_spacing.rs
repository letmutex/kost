//! Built-in AST rule enforcing spacing around commas (Ktlint rule 15).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around commas.
pub struct CommaSpacing;

impl AstRule for CommaSpacing {
    fn id(&self) -> &'static str {
        "comma-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[","]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        match spaces_before(node, source) {
            WhitespaceBefore::StartOfLine => {
                ctx.report(node, "Unexpected newline before ','");
            }
            WhitespaceBefore::Spaces(n) if n > 0 => {
                ctx.report(node, "Unexpected spacing before ','");
            }
            _ => {}
        }

        match spaces_after(node, source) {
            WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
                ctx.report(node, "Unexpected spacing after ','");
            }
            WhitespaceAfter::Spaces(0) => {
                ctx.report(node, "Missing spacing after ','");
            }
            WhitespaceAfter::Spaces(n) if n > 1 => {
                ctx.report(node, "Unexpected spacing after ','");
            }
            _ => {}
        }
    }
}
