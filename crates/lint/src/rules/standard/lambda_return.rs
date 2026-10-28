//! Built-in AST rule forbidding unnecessary labeled returns in lambdas (Ktlint rule 83).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::node_kind;

/// Rule that flags redundant labeled return statements at the end of a lambda.
pub struct LambdaReturn;

impl AstRule for LambdaReturn {
    fn id(&self) -> &'static str {
        "lambda-return"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["lambda_literal"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(statements_node) = find_statements_node(node) else {
            return;
        };

        let Some(last_statement) = find_last_non_comment_statement(statements_node) else {
            return;
        };

        if node_kind(last_statement) != "jump_expression" {
            return;
        }

        let (label_name, has_return_value) = parse_jump_expression(last_statement, source);
        let Some(label) = label_name else {
            return;
        };

        if !has_return_value {
            return;
        }

        let expected_label = find_innermost_lambda_label(ctx, source);
        if expected_label == Some(label) {
            ctx.report(last_statement, "Unnecessary return");
        }
    }
}

fn find_statements_node(lambda_node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = lambda_node.walk();
    lambda_node
        .children(&mut cursor)
        .find(|&child| node_kind(child) == "statements")
}

fn find_last_non_comment_statement(
    statements_node: tree_sitter::Node,
) -> Option<tree_sitter::Node> {
    let mut last = None;
    let mut cursor = statements_node.walk();
    for child in statements_node.children(&mut cursor) {
        if child.is_named()
            && !matches!(
                node_kind(child),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            last = Some(child);
        }
    }
    last
}

fn parse_jump_expression<'a>(
    jump_node: tree_sitter::Node,
    source: &'a [u8],
) -> (Option<&'a str>, bool) {
    let mut label_name = None;
    let mut has_value = false;
    let mut seen_label = false;

    let mut cursor = jump_node.walk();
    for child in jump_node.children(&mut cursor) {
        let kind = node_kind(child);
        if kind == "label" {
            if let Ok(text) = child.utf8_text(source) {
                let trimmed = text.trim_matches('@').trim();
                label_name = Some(trimmed);
            }
            seen_label = true;
        } else if seen_label
            && child.is_named()
            && !matches!(kind, "comment" | "line_comment" | "multiline_comment")
        {
            has_value = true;
        }
    }

    (label_name, has_value)
}

fn find_innermost_lambda_label<'a>(ctx: &LintCtx, source: &'a [u8]) -> Option<&'a str> {
    let parent = ctx.parent()?;

    if node_kind(parent) == "annotated_lambda" {
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            if node_kind(child) == "label"
                && let Ok(text) = child.utf8_text(source)
            {
                let trimmed = text.trim_matches('@').trim();
                return Some(trimmed);
            }
        }
    }

    find_enclosing_call_name(ctx, source)
}

fn find_enclosing_call_name<'a>(ctx: &LintCtx, source: &'a [u8]) -> Option<&'a str> {
    let mut level = 1;
    while let Some(node) = ctx.ancestor(level) {
        let kind = node_kind(node);
        if kind == "call_expression" {
            return extract_callee_name(node, source);
        }
        if matches!(
            kind,
            "function_declaration"
                | "property_declaration"
                | "class_declaration"
                | "object_declaration"
        ) {
            break;
        }
        level += 1;
    }
    None
}

fn extract_callee_name<'a>(call_node: tree_sitter::Node, source: &'a [u8]) -> Option<&'a str> {
    let first_child = call_node.named_child(0)?;
    match first_child.kind() {
        "simple_identifier" => first_child.utf8_text(source).ok(),
        "navigation_expression" => {
            let mut cursor = first_child.walk();
            for child in first_child.children(&mut cursor) {
                if child.kind() == "navigation_suffix" {
                    let mut suffix_cursor = child.walk();
                    for suffix_child in child.children(&mut suffix_cursor) {
                        if suffix_child.kind() == "simple_identifier" {
                            return suffix_child.utf8_text(source).ok();
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}
