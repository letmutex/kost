//! Built-in AST rule enforcing lambda literal formatting (Ktlint rule 81).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that formats lambda literals and enforces arrow and parameter newline conventions.
pub struct FunctionLiteral;

impl AstRule for FunctionLiteral {
    fn id(&self) -> &'static str {
        "function-literal"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["lambda_literal"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let (open_brace, params, arrow, statements, close_brace) = parse_lambda_structure(node);

        if let Some(arrow_node) = arrow {
            check_redundant_arrow(node, arrow_node, params, statements, ctx);
        }

        if let Some(params_node) = params {
            check_parameter_newlines(open_brace, params_node, arrow, ctx);
        }

        check_body_newlines(open_brace, params, arrow, statements, close_brace, ctx);
    }
}

type LambdaParts<'a> = (
    Option<tree_sitter::Node<'a>>,
    Option<tree_sitter::Node<'a>>,
    Option<tree_sitter::Node<'a>>,
    Option<tree_sitter::Node<'a>>,
    Option<tree_sitter::Node<'a>>,
);

fn parse_lambda_structure(node: tree_sitter::Node<'_>) -> LambdaParts<'_> {
    let mut open_brace = None;
    let mut params = None;
    let mut arrow = None;
    let mut statements = None;
    let mut close_brace = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "{" => open_brace = Some(child),
            "lambda_parameters" => params = Some(child),
            "->" => arrow = Some(child),
            "statements" => statements = Some(child),
            "}" => close_brace = Some(child),
            _ => {}
        }
    }

    (open_brace, params, arrow, statements, close_brace)
}

fn check_redundant_arrow(
    lambda_node: tree_sitter::Node,
    arrow_node: tree_sitter::Node,
    params: Option<tree_sitter::Node>,
    statements: Option<tree_sitter::Node>,
    ctx: &mut LintCtx,
) {
    let has_empty_params = params.is_none_or(|p| p.named_child_count() == 0);
    if !has_empty_params {
        return;
    }

    // If block after arrow is empty, arrow is required to disambiguate empty lambda { -> }
    let has_statements = statements.is_some_and(|s| s.named_child_count() > 0);
    if !has_statements {
        return;
    }

    // Direct branch bodies of when_entry, then, or else require arrow to parse as lambda
    if is_direct_branch_body(lambda_node) {
        return;
    }

    ctx.report(
        arrow_node,
        "Arrow is redundant when parameter list is empty",
    );
}

fn is_direct_branch_body(node: tree_sitter::Node) -> bool {
    let effective_parent = match node.parent() {
        Some(p) if p.kind() == "annotated_lambda" => p.parent(),
        other => other,
    };

    let Some(parent) = effective_parent else {
        return false;
    };

    if parent.kind() == "when_entry" {
        return true;
    }

    if parent.kind() == "control_structure_body"
        && let Some(grandparent) = parent.parent()
    {
        return matches!(grandparent.kind(), "if_expression" | "when_entry");
    }

    false
}

fn check_parameter_newlines(
    open_brace: Option<tree_sitter::Node>,
    params_node: tree_sitter::Node,
    arrow: Option<tree_sitter::Node>,
    ctx: &mut LintCtx,
) {
    let mut parameter_nodes: SmallVec<[tree_sitter::Node; 4]> = SmallVec::new();
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        if child.kind() == "parameter" || child.kind() == "variable_declaration" {
            parameter_nodes.push(child);
        }
    }

    if parameter_nodes.is_empty() {
        return;
    }

    let is_multiline = params_node.start_position().row != params_node.end_position().row;

    if is_multiline {
        for (index, param) in parameter_nodes.iter().enumerate() {
            let prev_line = if index == 0 {
                open_brace.map(|b| b.end_position().row)
            } else {
                Some(parameter_nodes[index - 1].end_position().row)
            };
            if prev_line == Some(param.start_position().row) {
                ctx.report(*param, "Newline expected before parameter");
            }
        }
        if let Some(arrow_node) = arrow
            && arrow_node.start_position().row == params_node.end_position().row
        {
            ctx.report(arrow_node, "Newline expected before arrow");
        }
    } else if let Some(first_param) = parameter_nodes.first() {
        if let Some(brace) = open_brace
            && brace.end_position().row != first_param.start_position().row
            && !has_preceding_comment(open_brace, params_node)
        {
            ctx.report(*first_param, "No newline expected before parameter");
        }
        let last_param = parameter_nodes.last().unwrap_or(first_param);
        if let Some(arrow_node) = arrow
            && arrow_node.start_position().row != last_param.end_position().row
        {
            ctx.report(*last_param, "No newline expected after parameter");
        }
    }
}

fn has_preceding_comment(
    open_brace: Option<tree_sitter::Node>,
    params_node: tree_sitter::Node,
) -> bool {
    let mut prev = params_node.prev_sibling();
    while let Some(sibling) = prev {
        if let Some(brace) = open_brace
            && sibling.id() == brace.id()
        {
            break;
        }
        if matches!(
            sibling.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            return true;
        }
        prev = sibling.prev_sibling();
    }
    false
}

fn check_body_newlines(
    open_brace: Option<tree_sitter::Node>,
    params: Option<tree_sitter::Node>,
    arrow: Option<tree_sitter::Node>,
    statements: Option<tree_sitter::Node>,
    close_brace: Option<tree_sitter::Node>,
    ctx: &mut LintCtx,
) {
    let Some(stmts) = statements else {
        return;
    };

    let is_body_multiline = stmts.start_position().row != stmts.end_position().row;
    let is_params_multiline =
        params.is_some_and(|p| p.start_position().row != p.end_position().row);

    let is_lambda_multiline = match (open_brace, close_brace) {
        (Some(open), Some(close)) => open.start_position().row != close.end_position().row,
        _ => is_body_multiline,
    };

    if let Some(arrow_node) = arrow {
        if (is_body_multiline || is_params_multiline)
            && arrow_node.end_position().row == stmts.start_position().row
        {
            ctx.report(arrow_node, "Newline expected after arrow");
        }
    } else if is_lambda_multiline
        && let Some(brace) = open_brace
        && brace.end_position().row == stmts.start_position().row
    {
        ctx.report(brace, "Newline expected after opening brace");
    }

    if is_lambda_multiline
        && let Some(rbrace) = close_brace
        && rbrace.start_position().row == stmts.end_position().row
    {
        ctx.report(rbrace, "Newline expected before closing brace");
    }
}
