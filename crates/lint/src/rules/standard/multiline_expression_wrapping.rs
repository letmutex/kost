//! Built-in AST rule enforcing multiline expression wrapping after '=' (Ktlint rule 55).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that forces multiline expressions following '=' to start on a new line.
pub struct MultilineExpressionWrapping;

impl AstRule for MultilineExpressionWrapping {
    fn id(&self) -> &'static str {
        "multiline-expression-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_body", "property_declaration", "assignment"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut eq_token = None;
        let mut expression_node = None;

        for child in node.children(&mut cursor) {
            if eq_token.is_none() {
                if child.kind() == "=" {
                    eq_token = Some(child);
                }
            } else if child.is_named()
                && !matches!(
                    child.kind(),
                    "line_comment" | "multiline_comment" | "comment"
                )
            {
                expression_node = Some(child);
                break;
            }
        }

        let (Some(eq_node), Some(expr_node)) = (eq_token, expression_node) else {
            return;
        };

        if expr_node.start_position().row == expr_node.end_position().row {
            return;
        }

        if matches!(
            expr_node.kind(),
            "string_literal"
                | "multiline_string_literal"
                | "lambda_literal"
                | "annotated_lambda"
                | "object_literal"
        ) {
            return;
        }

        if expr_node.kind() == "call_expression"
            && let Some(callee) = expr_node.child(0)
            && callee.end_position().row == eq_node.start_position().row
            && let Some(suffix) = expr_node.child(1)
            && suffix.kind() == "call_suffix"
            && suffix.children(&mut cursor).any(|c| {
                matches!(
                    c.kind(),
                    "value_arguments" | "annotated_lambda" | "lambda_literal"
                )
            })
        {
            return;
        }

        if expr_node.start_position().row == eq_node.start_position().row {
            ctx.report(eq_node, "Expected newline after '='");
        }
    }
}
