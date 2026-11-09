use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::lambda_ast::{
    find_enclosing_call_and_lambda, find_lambda_argument_index_in_call, find_param_index_in_lambda,
    lambda_has_explicit_params,
};
use super::scope_enclosing_lambda::resolve_lambda_type_for_call_and_lambda;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

pub fn resolve_lambda_param_from_call(
    ctx: InferenceContext<'_, '_, '_>,
    call_node: Node,
    lambda_node: Node,
    param_idx: usize,
) -> Option<KotlinType> {
    let (fn_ty, _) = resolve_lambda_type_for_call_and_lambda(
        ctx.index,
        ctx.current_url,
        call_node,
        lambda_node,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    )?;
    if let KotlinType::Function { params, .. } = fn_ty.unwrap_nullable_ref() {
        params.get(param_idx).cloned()
    } else {
        None
    }
}

/// Infer type of a lambda parameter (explicit or implicit `it`) at the given position
pub fn infer_lambda_param_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    param_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let lambda_node = super::lambda_ast::find_innermost_enclosing_lambda(param_node)?;
    let param_idx = find_param_index_in_lambda(param_node, lambda_node);
    let (fn_ty, _) = super::scope_enclosing_lambda::resolve_enclosing_lambda_type(
        index,
        current_url,
        lambda_node,
        pos,
        source,
        root,
        depth,
    )?;
    if let KotlinType::Function { params, .. } = fn_ty.unwrap_nullable_ref() {
        params.get(param_idx).cloned()
    } else {
        None
    }
}

/// Infer type of implicit `it` for a known call and lambda node
#[allow(clippy::too_many_arguments)]
pub fn infer_lambda_it_type_for_call(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_node: Node,
    lambda_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH || lambda_has_explicit_params(lambda_node) {
        return None;
    }
    let _arg_idx = find_lambda_argument_index_in_call(call_node, lambda_node);
    let mut tracer = super::trace::Tracer::Noop;
    let ctx =
        super::InferenceContext::new(index, current_url, pos, source, root, depth, &mut tracer);
    resolve_lambda_param_from_call(ctx, call_node, lambda_node, 0)
}

/// Infer type of implicit `it` inside a lambda
pub fn infer_lambda_it_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    it_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    let (call_node, lambda_node) = find_enclosing_call_and_lambda(it_node)?;
    infer_lambda_it_type_for_call(
        index,
        current_url,
        call_node,
        lambda_node,
        pos,
        source,
        root,
        depth,
    )
}

fn resolve_decl_annotated_expected_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    parent: Node,
    source: &str,
) -> Option<KotlinType> {
    let decl_node = if parent.kind() == "function_body" {
        parent.parent()?
    } else {
        parent
    };
    let annot = crate::parser::extract_type_annotation(&decl_node, source)?;
    let raw = super::typealias::resolve_type_alias_or_import(index, current_url, annot);
    let full = super::qualification::resolve_full_type_name(index, current_url, &raw);
    let parsed = super::types::parse_type_signature(&full)?;
    Some(super::qualification::qualify_type(
        index,
        current_url,
        &parsed,
    ))
}

pub fn resolve_expected_type_for_expr(
    index: &WorkspaceIndex,
    current_url: &Url,
    expr_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let mut curr = expr_node;
    while let Some(parent) = curr.parent() {
        let pk = parent.kind();
        if matches!(
            pk,
            "parenthesized_expression"
                | "annotated_lambda"
                | "control_structure_body"
                | "if_expression"
                | "when_entry"
                | "when_expression"
                | "try_expression"
                | "catch_block"
                | "jump_expression"
                | "return_expression"
        ) {
            curr = parent;
            continue;
        }
        if matches!(pk, "statements" | "lambda_literal") {
            let outer_lambda = if pk == "lambda_literal" {
                parent
            } else {
                parent.parent().filter(|p| p.kind() == "lambda_literal")?
            };
            let (last_expr, _) =
                super::lambda_ast::find_last_expression_in_lambda(outer_lambda, source)?;
            if last_expr.start_byte() <= expr_node.start_byte()
                && expr_node.end_byte() <= last_expr.end_byte()
            {
                let (outer_fn, _) = super::scope_enclosing_lambda::resolve_enclosing_lambda_type(
                    index,
                    current_url,
                    outer_lambda,
                    pos,
                    source,
                    root,
                    depth + 1,
                )?;
                if let KotlinType::Function { return_type, .. } = outer_fn.unwrap_nullable_ref() {
                    return Some((**return_type).clone());
                }
            }
            return None;
        }
        if matches!(
            pk,
            "property_declaration"
                | "variable_declaration"
                | "parameter"
                | "class_parameter"
                | "function_declaration"
                | "function_body"
        ) {
            return resolve_decl_annotated_expected_type(index, current_url, parent, source);
        }
        break;
    }
    None
}
