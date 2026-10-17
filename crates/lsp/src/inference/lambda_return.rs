use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::expression::infer_receiver_type_depth;
use super::lambda_ast::find_last_expression_in_lambda;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::syntax::ast_call_view::AstCallView;

fn match_lambda_node<'a>(node: Node<'a>, source: &'a str, lambda_text: &str) -> Option<Node<'a>> {
    let lambda_literal = AstCallView::unwrap_lambda(node)?;
    if let Ok(txt) = lambda_literal.utf8_text(source.as_bytes())
        && txt.trim() == lambda_text.trim()
    {
        return Some(lambda_literal);
    }
    None
}

fn find_call_direct_lambda<'a>(
    node: Node<'a>,
    source: &'a str,
    lambda_text: &str,
) -> Option<Node<'a>> {
    let view = AstCallView::from_node(node)?;
    if let Some(trailing_lambda) = view.trailing_lambda
        && let Some(matched) = match_lambda_node(trailing_lambda, source, lambda_text)
    {
        return Some(matched);
    }
    for arg in view.value_arguments {
        if let Some(arg_expr) = AstCallView::argument_expression(arg)
            && let Some(matched) = match_lambda_node(arg_expr, source, lambda_text)
        {
            return Some(matched);
        }
    }
    None
}

pub fn find_matching_lambda_in_ast<'a>(
    root: Node<'a>,
    source: &'a str,
    lambda_text: &str,
    pos: Position,
) -> Option<Node<'a>> {
    if let Some(pos_node) = crate::parser::find_node_at_position(root, pos) {
        let mut curr = Some(pos_node);
        while let Some(node) = curr {
            if let Some(matched) = match_lambda_node(node, source, lambda_text) {
                return Some(matched);
            }
            if node.kind() == "call_expression"
                && let Some(matched) = find_call_direct_lambda(node, source, lambda_text)
            {
                return Some(matched);
            }
            curr = node.parent();
        }
        let decl = crate::parser::find_enclosing_declaration_node(pos_node);
        if let Some(matched_lambda) = find_lambda_with_text(decl, source, lambda_text) {
            return Some(matched_lambda);
        }
        return None;
    }
    find_lambda_with_text(root, source, lambda_text)
}

fn find_lambda_with_text<'a>(
    node: Node<'a>,
    source: &'a str,
    lambda_text: &str,
) -> Option<Node<'a>> {
    if let Some(matched) = match_lambda_node(node, source, lambda_text) {
        return Some(matched);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named()
            && !matches!(
                child.kind(),
                "comment"
                    | "line_comment"
                    | "multiline_comment"
                    | "package_header"
                    | "import_header"
            )
            && let Some(res) = find_lambda_with_text(child, source, lambda_text)
        {
            return Some(res);
        }
    }
    None
}

/// Find lambda literal node and preceding argument index in call root
pub fn find_lambda_node_and_arg_idx(root: Node) -> Option<(Node, usize)> {
    find_lambda_in_node(root)
}

fn find_lambda_in_node(node: Node) -> Option<(Node, usize)> {
    if node.kind() == "lambda_literal" {
        let arg_idx = count_preceding_arguments(node);
        return Some((node, arg_idx));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(res) = find_lambda_in_node(child) {
            return Some(res);
        }
    }
    None
}

fn count_preceding_arguments(lambda_node: Node) -> usize {
    let mut top_call = None;
    let mut curr = lambda_node.parent();
    while let Some(p) = curr {
        if p.kind() == "call_expression" {
            top_call = Some(p);
        } else if matches!(p.kind(), "statements" | "source_file" | "function_body") {
            break;
        }
        curr = p.parent();
    }

    let Some(call) = top_call else { return 0 };

    let mut count = 0;
    count_arguments_before(call, lambda_node.start_byte(), &mut count);
    count
}

fn count_arguments_before(node: Node, before_byte: usize, count: &mut usize) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "value_arguments" {
            let mut inner = child.walk();
            for arg in child.children(&mut inner) {
                if arg.kind() == "value_argument" && arg.end_byte() <= before_byte {
                    *count += 1;
                }
            }
        } else if child.kind() == "call_expression" || child.kind() == "call_suffix" {
            count_arguments_before(child, before_byte, count);
        }
    }
}

/// Infer return type of a lambda body
pub fn infer_lambda_return_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    lambda_node: Node,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    infer_lambda_return_type_expected(index, current_url, lambda_node, source, root, depth, None)
}

/// Infer return type of a lambda body with downward expected function signature
pub fn infer_lambda_return_type_expected(
    index: &WorkspaceIndex,
    current_url: &Url,
    lambda_node: Node,
    source: &str,
    root: Option<Node>,
    depth: usize,
    expected_fn: Option<&KotlinType>,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let (expr_node, expr_text) = match find_last_expression_in_lambda(lambda_node, source) {
        Some(res) => res,
        None => return Some(KotlinType::UNIT),
    };

    let start_pos = crate::position::point_to_position(expr_node.start_position());

    if let Some(r) = root {
        let mut tracer = super::trace::Tracer::Noop;
        let inf_ctx = super::InferenceContext::new(
            index,
            current_url,
            start_pos,
            source,
            Some(r),
            depth,
            &mut tracer,
        );
        let mut ctx = super::ast_eval::AstEvalContext::new(inf_ctx, source);
        if let Some(efn) = expected_fn {
            ctx.apply_expected_function(lambda_node, efn);
        }
        if let Some(inferred) = super::ast_eval::infer_expression_node(&mut ctx, expr_node) {
            return Some(inferred);
        }
        // If AST evaluation fails on the actual node within root, do not fall back to
        // isolated snippet re-parsing which can trigger expensive whole-tree lambda searches.
        return None;
    }

    infer_receiver_type_depth(
        index,
        current_url,
        expr_text,
        start_pos,
        source,
        root,
        depth + 1,
    )
}
