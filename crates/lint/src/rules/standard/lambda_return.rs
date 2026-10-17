//! Built-in AST rule forbidding unnecessary labeled returns in lambdas (Ktlint rule 83).

use crate::context::LintCtx;
use crate::rule::AstRule;

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

        if last_statement.kind() != "jump_expression" {
            return;
        }

        let (label_name, has_return_value) = parse_jump_expression(last_statement, source);
        let Some(label) = label_name else {
            return;
        };

        if !has_return_value {
            return;
        }

        let expected_label = find_innermost_lambda_label(node, source);
        if expected_label == Some(label) {
            ctx.report(last_statement, "Unnecessary return");
        }
    }
}

fn find_statements_node(lambda_node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = lambda_node.walk();
    lambda_node
        .children(&mut cursor)
        .find(|&child| child.kind() == "statements")
}

fn find_last_non_comment_statement(
    statements_node: tree_sitter::Node,
) -> Option<tree_sitter::Node> {
    let mut last = None;
    let mut cursor = statements_node.walk();
    for child in statements_node.children(&mut cursor) {
        if child.is_named()
            && !matches!(
                child.kind(),
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
        if child.kind() == "label" {
            if let Ok(text) = child.utf8_text(source) {
                let trimmed = text.trim_matches('@').trim();
                label_name = Some(trimmed);
            }
            seen_label = true;
        } else if seen_label
            && child.is_named()
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            has_value = true;
        }
    }

    (label_name, has_value)
}

fn find_innermost_lambda_label<'a>(
    lambda_node: tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let parent = lambda_node.parent()?;

    if parent.kind() == "annotated_lambda" {
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            if child.kind() == "label"
                && let Ok(text) = child.utf8_text(source)
            {
                let trimmed = text.trim_matches('@').trim();
                return Some(trimmed);
            }
        }
    }

    find_enclosing_call_name(lambda_node, source)
}

fn find_enclosing_call_name<'a>(
    lambda_node: tree_sitter::Node,
    source: &'a [u8],
) -> Option<&'a str> {
    let mut current = lambda_node.parent();
    while let Some(node) = current {
        if node.kind() == "call_expression" {
            return extract_callee_name(node, source);
        }
        if matches!(
            node.kind(),
            "function_declaration"
                | "property_declaration"
                | "class_declaration"
                | "object_declaration"
        ) {
            break;
        }
        current = node.parent();
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
