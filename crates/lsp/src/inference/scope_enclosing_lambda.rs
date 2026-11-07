use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::expression::infer_receiver_type_depth;
use super::scope_lambda::{
    LambdaSolveCtx, resolve_member_lambda_type, resolve_toplevel_lambda_type,
};
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

fn dispatch_enclosing_lambda_callee<'a>(
    ctx: LambdaSolveCtx<'a>,
    callee_node: Node<'a>,
    callee_clean: &'a str,
) -> Option<(KotlinType, Option<SmolStr>)> {
    if callee_node.kind() == "navigation_expression" {
        let (rec_ty, method_name) = super::lambda_call_solve::resolve_navigation_callee_node(
            ctx.index,
            ctx.current_url,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth,
            callee_node,
        )?;
        let member_ctx = LambdaSolveCtx {
            receiver_type: Some(&rec_ty),
            ..ctx
        };
        resolve_member_lambda_type(&member_ctx, &rec_ty, method_name)
            .map(|kty| (kty, Some(SmolStr::new(method_name))))
    } else if let Some((rec_expr, method_name)) = callee_clean.rsplit_once('.') {
        let rec_ty = infer_receiver_type_depth(
            ctx.index,
            ctx.current_url,
            rec_expr,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        )?;
        let member_ctx = LambdaSolveCtx {
            receiver_type: Some(&rec_ty),
            ..ctx
        };
        resolve_member_lambda_type(&member_ctx, &rec_ty, method_name)
            .map(|kty| (kty, Some(SmolStr::new(method_name))))
    } else {
        resolve_toplevel_lambda_type(&ctx, callee_clean)
            .map(|kty| (kty, Some(SmolStr::new(callee_clean))))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_lambda_type_for_call_and_lambda(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_node: Node,
    l_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<(KotlinType, Option<SmolStr>)> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let arg_idx = super::lambda_ast::find_lambda_argument_index_in_call(call_node, l_node);
    let explicit_type_args = super::lambda_call_solve::extract_explicit_type_args_from_call(
        call_node,
        source,
        index,
        current_url,
    );

    let view = crate::syntax::ast_call_view::AstCallView::from_node(call_node);
    let callee_node = view
        .as_ref()
        .and_then(|v| v.callee_node)
        .or_else(|| call_node.child(0))?;
    let callee_raw = callee_node.utf8_text(source.as_bytes()).ok()?.trim();
    let call_info = super::call_parser::parse_call_expression(callee_raw);
    let callee_clean = call_info.callee.trim();
    let ast_args = super::lambda_ast::extract_call_argument_expressions(call_node, source);
    let (effective_value_args, effective_arg_idx) = if !call_info.value_args.is_empty() {
        (&call_info.value_args, call_info.value_args.len())
    } else {
        (&ast_args, arg_idx)
    };
    let is_trailing = view.as_ref().is_some_and(|v| {
        v.trailing_lambda.is_some_and(|tl| {
            tl.start_byte() <= l_node.start_byte() && l_node.end_byte() <= tl.end_byte()
        }) || !v.value_arguments.iter().any(|arg| {
            arg.start_byte() <= l_node.start_byte() && l_node.end_byte() <= arg.end_byte()
        })
    });
    let ctx = LambdaSolveCtx {
        index,
        current_url,
        receiver_type: None,
        explicit_type_args: &explicit_type_args,
        value_args: effective_value_args,
        arg_idx: effective_arg_idx,
        is_trailing,
        pos,
        source,
        root,
        depth,
    };

    dispatch_enclosing_lambda_callee(ctx, callee_node, callee_clean)
}

pub fn resolve_enclosing_lambda_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    lambda_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<(KotlinType, Option<SmolStr>)> {
    let (call_node, l_node) = super::lambda_ast::find_enclosing_call_and_lambda(lambda_node)?;
    resolve_lambda_type_for_call_and_lambda(
        index,
        current_url,
        call_node,
        l_node,
        pos,
        source,
        root,
        depth,
    )
}
