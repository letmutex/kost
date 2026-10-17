//! Built-in AST rule forbidding whitespace between function name and opening parenthesis (Ktlint rule 26).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, spaces_before};

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
        if node.kind() == "function_declaration" {
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
        if child.kind() == "simple_identifier" {
            name_node = Some(child);
        } else if child.kind() == "function_value_parameters" {
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

    if call_suffix.kind() != "call_suffix" {
        return;
    }

    let target = get_callee_name_node(callee);
    let mut cursor = call_suffix.walk();
    for child in call_suffix.children(&mut cursor) {
        if child.kind() == "type_arguments" {
            return;
        }
        if child.kind() == "value_arguments" {
            match spaces_before(child, source) {
                WhitespaceBefore::Spaces(count) if count > 0 => {
                    ctx.report(target, "Unexpected spacing between function name and '('");
                }
                WhitespaceBefore::StartOfLine => {
                    ctx.report(target, "Unexpected spacing between function name and '('");
                }
                _ => {}
            }
            return;
        }
    }
}

fn get_callee_name_node(callee: tree_sitter::Node) -> tree_sitter::Node {
    if callee.kind() == "navigation_expression" {
        let child_count = callee.child_count();
        if child_count > 0
            && let Some(suffix) = callee.child(child_count - 1)
            && suffix.kind() == "navigation_suffix"
        {
            let mut cursor = suffix.walk();
            for child in suffix.children(&mut cursor) {
                if child.kind() == "simple_identifier" {
                    return child;
                }
            }
        }
    }
    callee
}
