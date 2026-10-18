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
        let Some(parent) = ctx.parent() else {
            return;
        };
        let op = node.kind();
        let is_valid_parent = match op {
            "finally" => parent.kind() == "finally_block",
            "catch" => parent.kind() == "catch_block",
            "try" => parent.kind() == "try_expression",
            "if" => parent.kind() == "if_expression",
            "else" => matches!(parent.kind(), "if_expression" | "when_entry"),
            "while" => matches!(parent.kind(), "while_statement" | "do_while_statement"),
            "for" => parent.kind() == "for_statement",
            "when" => parent.kind() == "when_expression",
            "do" => parent.kind() == "do_while_statement",
            _ => false,
        };
        if !is_valid_parent {
            return;
        }

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
