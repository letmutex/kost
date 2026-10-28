//! Built-in AST rule forbidding unnecessary parentheses before trailing lambda (Ktlint rule 76).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that flags empty parentheses preceding a trailing lambda (`foo() { ... }`).
pub struct UnnecessaryParenthesesBeforeTrailingLambda;

impl AstRule for UnnecessaryParenthesesBeforeTrailingLambda {
    fn id(&self) -> &'static str {
        "unnecessary-parentheses-before-trailing-lambda"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let (arguments, lambda) = find_call_arguments_and_trailing_lambda(node);

        if let (Some(value_arguments), Some(trailing_lambda)) = (arguments, lambda)
            && trailing_lambda.start_byte() >= value_arguments.end_byte()
            && is_empty_value_arguments(value_arguments)
            && !is_misparsed_function_declaration(ctx, source)
        {
            ctx.report(
                value_arguments,
                "Unnecessary parentheses before trailing lambda",
            );
        }
    }
}

fn find_call_arguments_and_trailing_lambda(
    node: tree_sitter::Node,
) -> (Option<tree_sitter::Node>, Option<tree_sitter::Node>) {
    let mut arguments = None;
    let mut lambda = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match crate::util::node_kind(child) {
            "value_arguments" => arguments = Some(child),
            "lambda_literal" | "annotated_lambda" => lambda = Some(child),
            "call_suffix" => {
                let mut suffix_cursor = child.walk();
                for suffix_child in child.children(&mut suffix_cursor) {
                    match crate::util::node_kind(suffix_child) {
                        "value_arguments" => arguments = Some(suffix_child),
                        "lambda_literal" | "annotated_lambda" => lambda = Some(suffix_child),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    (arguments, lambda)
}

fn is_empty_value_arguments(arguments_node: tree_sitter::Node) -> bool {
    let mut cursor = arguments_node.walk();
    for child in arguments_node.children(&mut cursor) {
        let kind = crate::util::node_kind(child);
        if kind == "(" || kind == ")" {
            continue;
        }
        if child.is_named() || matches!(kind, "line_comment" | "multiline_comment" | "comment") {
            return false;
        }
    }
    true
}

fn is_misparsed_function_declaration(ctx: &LintCtx, source: &[u8]) -> bool {
    let mut level = 1;
    while let Some(parent) = ctx.ancestor(level) {
        let parent_kind = crate::util::node_kind(parent);
        if parent_kind == "infix_expression" {
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if crate::util::node_kind(child) == "simple_identifier"
                    && child.utf8_text(source) == Ok("fun")
                {
                    return true;
                }
            }
        }
        if !matches!(parent_kind, "navigation_expression" | "postfix_expression") {
            break;
        }
        level += 1;
    }
    false
}
