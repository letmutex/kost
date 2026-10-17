//! Built-in AST rule enforcing spacing after control flow keywords (Ktlint rule 19).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces proper spacing after Kotlin keywords.
pub struct KeywordSpacing;

impl AstRule for KeywordSpacing {
    fn id(&self) -> &'static str {
        "keyword-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "if", "else", "while", "for", "when", "try", "catch", "finally", "do",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let op = node.kind();

        match spaces_after(node, source) {
            WhitespaceAfter::Spaces(0) => {
                let message = crate::concat_str!("Missing spacing after '", op, "'");
                ctx.report(node, message);
            }
            WhitespaceAfter::Spaces(n) if n > 1 => {
                let message = crate::concat_str!("Unexpected spacing after '", op, "'");
                ctx.report(node, message);
            }
            _ => {}
        }
    }
}
