//! Built-in AST rule enforcing zero spaces around unary operators (Ktlint rule 24).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces zero spaces between unary operators and their operands.
pub struct UnaryOpSpacing;

impl AstRule for UnaryOpSpacing {
    fn id(&self) -> &'static str {
        "unary-op-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["prefix_expression", "postfix_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "prefix_expression" {
            check_prefix_expression(node, source, ctx);
        } else {
            check_postfix_expression(node, source, ctx);
        }
    }
}

fn check_prefix_expression(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let Some(operator_node) = node.child(0) else {
        return;
    };
    if !matches!(operator_node.kind(), "!" | "+" | "-" | "++" | "--") {
        return;
    }

    match spaces_after(operator_node, source) {
        WhitespaceAfter::Spaces(count) if count > 0 => {
            let op_str = operator_node.kind();
            let message = crate::concat_str!("Unexpected spacing after '", op_str, "'");
            ctx.report(operator_node, message);
        }
        WhitespaceAfter::EndOfLine { .. } => {
            let op_str = operator_node.kind();
            let message = crate::concat_str!("Unexpected spacing after '", op_str, "'");
            ctx.report(operator_node, message);
        }
        _ => {}
    }
}

fn check_postfix_expression(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let child_count = node.child_count();
    if child_count < 2 {
        return;
    }
    let Some(operator_node) = node.child(child_count - 1) else {
        return;
    };
    if !matches!(operator_node.kind(), "++" | "--" | "!!") {
        return;
    }

    match spaces_before(operator_node, source) {
        WhitespaceBefore::Spaces(count) if count > 0 => {
            let op_str = operator_node.kind();
            let message = crate::concat_str!("Unexpected spacing before '", op_str, "'");
            ctx.report(operator_node, message);
        }
        WhitespaceBefore::StartOfLine => {
            let op_str = operator_node.kind();
            let message = crate::concat_str!("Unexpected spacing before '", op_str, "'");
            ctx.report(operator_node, message);
        }
        _ => {}
    }
}
