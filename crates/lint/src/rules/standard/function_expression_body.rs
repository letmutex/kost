//! Built-in AST rule enforcing function expression bodies (Ktlint rule 52).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces replacing single-return block bodies with expression bodies.
pub struct FunctionExpressionBody;

impl AstRule for FunctionExpressionBody {
    fn id(&self) -> &'static str {
        "function-expression-body"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_body" {
                check_function_body(child, ctx);
                break;
            }
        }
    }
}

fn check_function_body(body_node: tree_sitter::Node, ctx: &mut LintCtx) {
    let mut cursor = body_node.walk();
    let mut single_return = false;
    let mut statement_count = 0;

    for child in body_node.children(&mut cursor) {
        if child.kind() == "statements" {
            let mut stmt_cursor = child.walk();
            for stmt in child.children(&mut stmt_cursor) {
                if !stmt.is_named()
                    || matches!(
                        stmt.kind(),
                        "line_comment" | "multiline_comment" | "comment"
                    )
                {
                    continue;
                }
                statement_count += 1;
                if stmt.kind() == "jump_expression" && has_return_value(stmt) {
                    single_return = true;
                }
            }
        }
    }

    if statement_count == 1 && single_return {
        ctx.report(
            body_node,
            "Function body should be replaced with body expression",
        );
    }
}

fn has_return_value(jump_node: tree_sitter::Node) -> bool {
    let mut cursor = jump_node.walk();
    let mut has_return = false;
    let mut has_value = false;

    for child in jump_node.children(&mut cursor) {
        if child.kind() == "return" {
            has_return = true;
        } else if child.is_named()
            && !matches!(
                child.kind(),
                "line_comment" | "multiline_comment" | "comment"
            )
        {
            has_value = true;
        }
    }

    has_return && has_value
}
