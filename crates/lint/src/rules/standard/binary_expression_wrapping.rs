//! Built-in AST rule enforcing binary expression wrapping (Ktlint rule 43).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces operators in multiline binary expressions start continuation lines.
pub struct BinaryExpressionWrapping;

impl AstRule for BinaryExpressionWrapping {
    fn id(&self) -> &'static str {
        "binary-expression-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "binary_expression",
            "additive_expression",
            "multiplicative_expression",
            "comparison_expression",
            "equality_expression",
            "conjunction_expression",
            "disjunction_expression",
            "infix_expression",
            "as_expression",
            "check_expression",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let Some(left_operand) = node.child(0) else {
            return;
        };

        let mut cursor = node.walk();
        let mut operator_node = None;
        let mut right_operand = None;

        for child in node.children(&mut cursor) {
            if child == left_operand {
                continue;
            }
            if operator_node.is_none() && is_binary_operator(node.kind(), child) {
                operator_node = Some(child);
            } else if operator_node.is_some()
                && child.is_named()
                && !matches!(
                    child.kind(),
                    "line_comment" | "multiline_comment" | "comment"
                )
            {
                right_operand = Some(child);
                break;
            }
        }

        let (Some(op_node), Some(right_node)) = (operator_node, right_operand) else {
            return;
        };

        if op_node.start_position().row < right_node.start_position().row {
            let operator_text = op_node.utf8_text(source).unwrap_or(op_node.kind());
            let message = crate::concat_str!("Line must not end with \"", operator_text, "\"");
            ctx.report(op_node, message);
        }
    }
}

fn is_binary_operator(parent_kind: &str, child: tree_sitter::Node) -> bool {
    let kind = child.kind();
    if parent_kind == "infix_expression" && kind == "simple_identifier" {
        return true;
    }
    matches!(
        kind,
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "&&"
            | "||"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | "<="
            | ">"
            | ">="
            | "in"
            | "!in"
            | "is"
            | "!is"
            | "as"
            | "as?"
    )
}
