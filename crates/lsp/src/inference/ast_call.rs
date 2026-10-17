use tower_lsp::lsp_types::Position;
use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::types::KotlinType;
use crate::syntax::ast_call_view::AstCallView;

fn find_member_access_operator<'a>(
    suffix_node: Node<'a>,
    source: &'a str,
) -> Option<(Node<'a>, &'a str)> {
    let mut cursor = suffix_node.walk();
    for child in suffix_node.children(&mut cursor) {
        if let Some(text) = source.get(child.start_byte()..child.end_byte()) {
            let trimmed = text.trim();
            if trimmed == "." || trimmed == "?." || trimmed == "::" {
                return Some((child, trimmed));
            }
        }
    }
    None
}

/// Infer navigation or call invocation
pub fn infer_invocation(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    match node.kind() {
        "navigation_expression" => infer_navigation_node(ctx, node),
        "call_expression" => infer_call_node(ctx, node),
        _ => None,
    }
}

/// Infer navigation expression (e.g. `obj.prop`, `obj?.prop`, `obj.method()`)
pub fn infer_navigation_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let lhs_node = node.named_child(0)?;
    let suffix_node = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "navigation_suffix")?;
    let (op_node, op_text) = find_member_access_operator(suffix_node, ctx.node_source)?;
    if op_text == "::" {
        return None;
    }
    let is_safe = op_text == "?.";
    let mut sub_ctx = ctx.sub();
    let lhs_ty = infer_expression_node(&mut sub_ctx, lhs_node)?;
    let member_expr = ctx
        .node_source
        .get(op_node.end_byte()..node.end_byte())?
        .trim();
    let is_lhs_nullable = lhs_ty.is_nullable();
    let non_null_lhs = lhs_ty.make_non_null();
    let nav_pos = crate::position::point_to_position(suffix_node.start_position());
    let member_res = super::member::resolve_member_type_kt_depth_traced(
        ctx.sub_with_pos(nav_pos),
        &non_null_lhs,
        member_expr,
    )?;
    let mut kt_res = member_res;
    if is_safe || is_lhs_nullable {
        kt_res = kt_res.make_nullable();
    }
    Some(kt_res)
}

fn find_call_pos(call_view: Option<&AstCallView>, fallback: Node) -> Position {
    if let Some(view) = call_view {
        if let Some(trailing_lambda) = view.trailing_lambda {
            let target = AstCallView::unwrap_lambda(trailing_lambda).unwrap_or(trailing_lambda);
            return crate::position::point_to_position(target.start_position());
        }
        for arg in &view.value_arguments {
            if let Some(arg_expr) = AstCallView::argument_expression(*arg)
                && let Some(target) = AstCallView::unwrap_lambda(arg_expr)
            {
                return crate::position::point_to_position(target.start_position());
            }
        }
    }
    crate::position::point_to_position(fallback.start_position())
}

/// Infer call expression (e.g. `foo()`, `obj.method()`, or call with lambda)
pub fn infer_call_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let call_view = AstCallView::from_node(node);
    let callee_node = call_view
        .as_ref()
        .and_then(|v| v.callee_node)
        .or_else(|| node.child(0))?;
    if callee_node.kind() == "navigation_expression" {
        let lhs_node = callee_node.named_child(0)?;
        let suffix_node = callee_node
            .children(&mut callee_node.walk())
            .find(|c| c.kind() == "navigation_suffix")?;
        let (op_node, op_text) = find_member_access_operator(suffix_node, ctx.node_source)?;
        let is_safe = op_text == "?.";

        let mut sub_ctx = ctx.sub();
        let lhs_ty = infer_expression_node(&mut sub_ctx, lhs_node)?;
        let member_expr = ctx
            .node_source
            .get(op_node.end_byte()..node.end_byte())?
            .trim();

        let is_lhs_nullable = lhs_ty.is_nullable();
        let non_null_lhs = lhs_ty.make_non_null();
        let call_pos = find_call_pos(call_view.as_ref(), suffix_node);
        let member_res = super::member::resolve_member_type_kt_depth_traced(
            ctx.sub_with_pos(call_pos),
            &non_null_lhs,
            member_expr,
        )?;
        let mut kt_res = member_res;
        if is_safe || is_lhs_nullable {
            kt_res = kt_res.make_nullable();
        }
        Some(kt_res)
    } else {
        let call_text = ctx.node_text(node)?;
        let call_pos = find_call_pos(call_view.as_ref(), node);
        if super::lambda_call::is_call_with_lambda(call_text) {
            super::lambda_call::resolve_higher_order_call(
                ctx.ctx.reborrow().at_pos(call_pos),
                None,
                call_text,
            )
        } else {
            super::function::find_function_return_type_depth(
                ctx.index,
                ctx.current_url,
                call_text,
                call_pos,
                ctx.source,
                ctx.root,
                ctx.depth,
            )
        }
    }
}
