//! Built-in AST rule enforcing spacing around binary operators (Ktlint rule 20).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around binary operators.
pub struct OpSpacing;

impl AstRule for OpSpacing {
    fn id(&self) -> &'static str {
        "op-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "=", "+=", "-=", "*=", "/=", "%=", "+", "-", "*", "/", "%", "==", "!=", "===", "!==",
            "<", "<=", ">", ">=", "&&", "||", "?:",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let parent_kind = ctx.parent().map(|p| p.kind()).unwrap_or("");
        if matches!(
            parent_kind,
            "prefix_expression"
                | "postfix_expression"
                | "spread_expression"
                | "wildcard_import"
                | "type_arguments"
                | "type_parameters"
                | "type_projection"
                | "super_expression"
        ) || (parent_kind == "value_argument" && node.kind() == "*")
        {
            return;
        }

        let op = node.kind();

        match spaces_before(node, source) {
            WhitespaceBefore::StartOfLine => {}
            WhitespaceBefore::Spaces(0) => {
                let message = crate::concat_str!("Missing spacing before '", op, "'");
                ctx.report(node, message);
            }
            WhitespaceBefore::Spaces(n) if n > 1 => {
                let message = crate::concat_str!("Unexpected spacing before '", op, "'");
                ctx.report(node, message);
            }
            _ => {}
        }

        match spaces_after(node, source) {
            WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
                let message = crate::concat_str!("Unexpected spacing after '", op, "'");
                ctx.report(node, message);
            }
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
