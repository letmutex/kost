//! Built-in AST rule forbidding whitespace between function name and opening parenthesis (Ktlint rule 26).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, node_kind, spaces_before};

/// Rule that forbids spacing between a function identifier and its parameter/argument opening parenthesis.
pub struct FunctionNameSpacing;

impl AstRule for FunctionNameSpacing {
    fn id(&self) -> &'static str {
        "spacing-between-function-name-and-opening-parenthesis"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "call_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node_kind(node) == "function_declaration" {
            check_function_declaration(node, source, ctx);
        } else {
            check_call_expression(node, source, ctx);
        }
    }
}

fn check_function_declaration(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let mut cursor = node.walk();
    let mut name_node = None;

    for child in node.children(&mut cursor) {
        let kind = node_kind(child);
        if kind == "simple_identifier" {
            name_node = Some(child);
        } else if kind == "function_value_parameters" {
            if let Some(name) = name_node {
                match spaces_before(child, source) {
                    WhitespaceBefore::Spaces(count) if count > 0 => {
                        ctx.report(name, "Unexpected spacing between function name and '('");
                    }
                    WhitespaceBefore::StartOfLine => {
                        ctx.report(name, "Unexpected spacing between function name and '('");
                    }
                    _ => {}
                }
            }
            break;
        }
    }
}

fn check_call_expression(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let Some(callee) = node.child(0) else {
        return;
    };
    let Some(call_suffix) = node.child(1) else {
        return;
    };

    if callee.end_byte() == call_suffix.start_byte() {
        return;
    }

    if node_kind(call_suffix) != "call_suffix" {
        return;
    }

    let mut cursor = call_suffix.walk();
    for child in call_suffix.children(&mut cursor) {
        let kind = node_kind(child);
        if matches!(
            kind,
            "type_arguments" | "annotated_lambda" | "lambda_literal"
        ) {
            return;
        }
        if kind == "value_arguments" {
            if let WhitespaceBefore::Spaces(count) = spaces_before(child, source)
                && count > 0
                && let Some(target) = get_callee_name_node(callee)
            {
                ctx.report(target, "Unexpected spacing between function name and '('");
            }
            return;
        }
    }
}

fn get_callee_name_node(callee: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let callee_kind = node_kind(callee);
    if callee_kind == "simple_identifier" {
        return Some(callee);
    }
    if callee_kind == "navigation_expression" {
        let child_count = callee.child_count();
        if child_count > 0
            && let Some(suffix) = callee.child(child_count - 1)
            && node_kind(suffix) == "navigation_suffix"
        {
            let mut cursor = suffix.walk();
            for child in suffix.children(&mut cursor) {
                if node_kind(child) == "simple_identifier" {
                    return Some(child);
                }
            }
        }
    }
    None
}
