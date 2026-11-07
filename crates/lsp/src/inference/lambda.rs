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
    let (call_node, lambda_node) = find_enclosing_call_and_lambda(param_node)?;
    let param_idx = find_param_index_in_lambda(param_node, lambda_node);

    let mut tracer = super::trace::Tracer::Noop;
    let ctx =
        super::InferenceContext::new(index, current_url, pos, source, root, depth, &mut tracer);
    resolve_lambda_param_from_call(ctx, call_node, lambda_node, param_idx)
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
