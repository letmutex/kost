use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::callable_reference::resolve_callable_reference;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::syntax::ast_call_view::AstCallView;

pub(crate) fn find_member_access_operator<'a>(
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

/// Infer navigation expression (e.g. `obj.prop`, `obj?.prop`, `obj.method()`, `Type::member`)
pub fn infer_navigation_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let lhs_node = node.named_child(0)?;
    let suffix_node = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "navigation_suffix")?;
    let (op_node, op_text) = find_member_access_operator(suffix_node, ctx.node_source)?;
    if op_text == "::" {
        let member_node = suffix_node
            .children(&mut suffix_node.walk())
            .find(|c| c.kind() == "simple_identifier")?;
        let member_name = ctx.node_text(member_node)?;
        let lhs_resolved = resolve_callable_ref_lhs(ctx, lhs_node)?;
        return resolve_callable_reference(ctx, Some(lhs_resolved), member_name);
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
    let mut member_res = super::member::resolve_member_type_kt_depth_traced(
        ctx.sub_with_pos(nav_pos),
        &non_null_lhs,
        member_expr,
    )?;
    if is_safe || is_lhs_nullable {
        member_res = member_res.make_nullable();
    }
    Some(member_res)
}

pub fn infer_callable_reference_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut lhs_node = None;
    let mut member_node = None;
    let mut seen_colons = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "::" {
            seen_colons = true;
        } else if !seen_colons && child.is_named() && lhs_node.is_none() {
            lhs_node = Some(child);
        } else if seen_colons && child.kind() == "simple_identifier" && member_node.is_none() {
            member_node = Some(child);
        }
    }
    let member_name = ctx.node_text(member_node?)?;
    let lhs_resolved = match lhs_node {
        Some(lhs) => Some(resolve_callable_ref_lhs(ctx, lhs)?),
        None => None,
    };
    resolve_callable_reference(ctx, lhs_resolved, member_name)
}

fn is_singleton_object_type(index: &WorkspaceIndex, current_url: &Url, ty: &KotlinType) -> bool {
    let short = ty.base_name().rsplit('.').next().unwrap_or(ty.base_name());
    index.name_to_decls.get(short).is_some_and(|decls| {
        decls.iter().any(|(url, decl)| {
            decl.kind == SymbolKind::OBJECT
                && decl.container_name.is_none()
                && super::qualification::is_declaration_visible(
                    index,
                    current_url,
                    url.as_ref(),
                    decl,
                )
        })
    })
}

fn is_known_type_identifier(index: &WorkspaceIndex, ident: &str) -> bool {
    if super::model::PrimitiveType::from_name(ident).is_some()
        || super::types::standard::StandardInterface::from_name(ident).is_some()
        || index.type_aliases.contains_key(ident)
    {
        return true;
    }
    let short = ident.rsplit('.').next().unwrap_or(ident);
    index.name_to_decls.get(short).is_some_and(|decls| {
        decls.iter().any(|(_, decl)| {
            matches!(
                decl.kind,
                SymbolKind::CLASS
                    | SymbolKind::INTERFACE
                    | SymbolKind::ENUM
                    | SymbolKind::OBJECT
                    | SymbolKind::TYPE_PARAMETER
            )
        })
    })
}

pub(crate) fn resolve_callable_ref_lhs(
    ctx: &mut AstEvalContext,
    lhs: Node,
) -> Option<(KotlinType, bool)> {
    if crate::inference::types::ast_type::is_type_node(&lhs) {
        let parsed =
            crate::inference::types::ast_node_to_kotlin_type(&lhs, ctx.node_source, &[], false)?;
        let qualified = super::qualification::qualify_type(ctx.index, ctx.current_url, &parsed);
        let is_unbound = !is_singleton_object_type(ctx.index, ctx.current_url, &qualified);
        return Some((qualified, is_unbound));
    }
    if matches!(lhs.kind(), "simple_identifier" | "type_identifier") {
        let lhs_text = ctx.node_text(lhs)?;
        if let Some(local_ty) = super::variable::resolve_local_receiver(
            ctx.index,
            ctx.current_url,
            lhs_text,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        ) {
            return Some((local_ty, false));
        }
        if is_known_type_identifier(ctx.index, lhs_text) {
            let full =
                super::qualification::resolve_full_type_name(ctx.index, ctx.current_url, lhs_text);
            let parsed = super::types::parse_type_signature(&full)?;
            let qualified = super::qualification::qualify_type(ctx.index, ctx.current_url, &parsed);
            let is_unbound = !is_singleton_object_type(ctx.index, ctx.current_url, &qualified);
            return Some((qualified, is_unbound));
        }
    }
    let expr_ty = infer_expression_node(&mut ctx.sub(), lhs)?;
    Some((expr_ty, false))
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

fn infer_view_arg_types(ctx: &mut AstEvalContext, view: &AstCallView) -> Vec<Option<KotlinType>> {
    view.value_arguments
        .iter()
        .map(|arg| {
            let expr_node = AstCallView::argument_expression(*arg).unwrap_or(*arg);
            let mut sub_ctx = ctx.sub();
            infer_expression_node(&mut sub_ctx, expr_node)
        })
        .collect()
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
        let member_res = if !super::lambda_call::is_call_with_lambda(member_expr)
            && let Some(ref view) = call_view
        {
            let call_info = super::call_parser::parse_call_expression(member_expr);
            let inferred_args = infer_view_arg_types(ctx, view);
            super::member::resolve_member_call_info_depth_traced(
                ctx.sub_with_pos(call_pos),
                &non_null_lhs,
                member_expr,
                &call_info,
                &inferred_args,
            )?
        } else {
            super::member::resolve_member_type_kt_depth_traced(
                ctx.sub_with_pos(call_pos),
                &non_null_lhs,
                member_expr,
            )?
        };
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
