//! Built-in AST rule enforcing call expression wrapping (Ktlint rule 44).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces consistent line breaks in multiline call expressions.
pub struct CallExpressionWrapping;

impl AstRule for CallExpressionWrapping {
    fn id(&self) -> &'static str {
        "call-expression-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "call_suffix" => {
                    let mut suffix_cursor = child.walk();
                    for suffix_child in child.children(&mut suffix_cursor) {
                        check_call_child(suffix_child, source, ctx);
                    }
                }
                _ => check_call_child(child, source, ctx),
            }
        }
    }
}

fn check_call_child(child: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    match child.kind() {
        "value_arguments" => check_multiline_value_arguments(child, source, ctx),
        "lambda_literal" | "annotated_lambda" => check_multiline_lambda(child, source, ctx),
        _ => {}
    }
}

fn check_multiline_value_arguments(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if node.start_position().row == node.end_position().row {
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "(" {
            if !has_newline_after(child, node, source) {
                ctx.report(child, "Expected new line after '('");
            }
        } else if child.kind() == ")"
            && spaces_before(child, source) != WhitespaceBefore::StartOfLine
        {
            ctx.report(child, "Expected new line before ')'");
        }
    }
}

fn check_multiline_lambda(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if node.start_position().row == node.end_position().row {
        return;
    }

    let lambda_literal = if node.kind() == "annotated_lambda" {
        let mut child_cursor = node.walk();
        node.children(&mut child_cursor)
            .find(|child| child.kind() == "lambda_literal")
            .unwrap_or(node)
    } else {
        node
    };

    let mut opening_brace = None;
    let mut closing_brace = None;
    let mut arrow_token = None;

    find_lambda_tokens(
        lambda_literal,
        &mut opening_brace,
        &mut closing_brace,
        &mut arrow_token,
    );

    if let Some(arrow) = arrow_token {
        if !has_newline_after(arrow, lambda_literal, source) {
            ctx.report(arrow, "Expected new line after '->'");
        }
    } else if let Some(brace) = opening_brace
        && !has_newline_after(brace, lambda_literal, source)
    {
        ctx.report(brace, "Expected new line after '{'");
    }

    if let Some(brace) = closing_brace
        && spaces_before(brace, source) != WhitespaceBefore::StartOfLine
    {
        ctx.report(brace, "Expected new line before '}'");
    }
}

fn has_newline_after(
    delimiter: tree_sitter::Node,
    container: tree_sitter::Node,
    source: &[u8],
) -> bool {
    let mut cursor = container.walk();
    let mut found_delimiter = false;
    for child in container.children(&mut cursor) {
        if !found_delimiter {
            if child == delimiter
                || (child.kind() == "lambda_parameters" && delimiter.parent() == Some(child))
            {
                found_delimiter = true;
            }
            continue;
        }
        if matches!(
            child.kind(),
            "line_comment" | "multiline_comment" | "comment"
        ) {
            continue;
        }
        if child.kind() == "}" || child.kind() == ")" {
            return child.start_position().row > delimiter.start_position().row;
        }
        if child.kind() == "statements" {
            let mut stmt_cursor = child.walk();
            for stmt_child in child.children(&mut stmt_cursor) {
                if matches!(
                    stmt_child.kind(),
                    "line_comment" | "multiline_comment" | "comment"
                ) {
                    continue;
                }
                return stmt_child.start_position().row > delimiter.start_position().row;
            }
            continue;
        }
        return child.start_position().row > delimiter.start_position().row;
    }
    matches!(
        spaces_after(delimiter, source),
        WhitespaceAfter::EndOfLine { .. }
    )
}

fn find_lambda_tokens<'a>(
    node: tree_sitter::Node<'a>,
    opening_brace: &mut Option<tree_sitter::Node<'a>>,
    closing_brace: &mut Option<tree_sitter::Node<'a>>,
    arrow_token: &mut Option<tree_sitter::Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "{" => *opening_brace = Some(child),
            "}" => *closing_brace = Some(child),
            "->" => *arrow_token = Some(child),
            "lambda_parameters" => {
                let mut param_cursor = child.walk();
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "->" {
                        *arrow_token = Some(param_child);
                    }
                }
            }
            _ => {}
        }
    }
}
