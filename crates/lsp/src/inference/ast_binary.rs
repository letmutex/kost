use tower_lsp::lsp_types::Url;
use tree_sitter::Node;

use super::ast_call::find_member_access_operator;
use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::model::PrimitiveType;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

fn try_infer_misparsed_generic_call(
    ctx: &mut AstEvalContext,
    node: Node,
    left_node: Node,
) -> Option<KotlinType> {
    let node_text = ctx.node_text(node)?;
    if !node_text.contains('<') || (!node_text.ends_with(')') && !node_text.ends_with('}')) {
        return None;
    }
    let call_info = super::call_parser::parse_call_expression(node_text);
    if call_info.explicit_type_args.is_empty()
        || !call_info
            .explicit_type_args
            .iter()
            .all(|arg| super::types::parse_type_signature(arg).is_some())
    {
        return None;
    }

    let mut first_lhs = left_node;
    while first_lhs.kind() == "comparison_expression" {
        if let Some(child) = first_lhs.named_child(0) {
            first_lhs = child;
        } else {
            break;
        }
    }

    let call_pos = crate::position::point_to_position(first_lhs.start_position());
    if first_lhs.kind() == "navigation_expression" {
        let lhs_node = first_lhs.named_child(0)?;
        let suffix_node = first_lhs
            .children(&mut first_lhs.walk())
            .find(|child| child.kind() == "navigation_suffix")?;
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
        let mut resolved = super::member::resolve_member_type_kt_depth_traced(
            ctx.sub_with_pos(call_pos),
            &non_null_lhs,
            member_expr,
        )?;
        if is_safe || is_lhs_nullable {
            resolved = resolved.make_nullable();
        }
        Some(resolved)
    } else if first_lhs.kind() == "simple_identifier" {
        if super::lambda_call::is_call_with_lambda(node_text) {
            super::lambda_call::resolve_higher_order_call(
                ctx.ctx.reborrow().at_pos(call_pos),
                None,
                node_text,
            )
        } else {
            super::function::find_function_return_type_depth(
                ctx.index,
                ctx.current_url,
                node_text,
                call_pos,
                ctx.source,
                ctx.root,
                ctx.depth,
            )
        }
    } else {
        None
    }
}

pub fn infer_binary_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut cursor = node.walk();
    let non_comment: Vec<Node> = node
        .children(&mut cursor)
        .filter(|c| !matches!(c.kind(), "comment" | "line_comment" | "multiline_comment"))
        .collect();
    if non_comment.len() < 3 {
        return None;
    }
    let left_node = non_comment[0];
    let op_node = non_comment[1];
    let right_node = non_comment[2];
    let op = ctx.node_text(op_node)?;
    if matches!(op, "<" | ">")
        && let Some(call_ty) = try_infer_misparsed_generic_call(ctx, node, left_node)
    {
        return Some(call_ty);
    }
    infer_binary_operator(ctx, left_node, op, right_node)
}

pub fn infer_elvis_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut cursor = node.walk();
    let non_comment: Vec<Node> = node
        .children(&mut cursor)
        .filter(|c| !matches!(c.kind(), "comment" | "line_comment" | "multiline_comment"))
        .collect();
    let left_node = non_comment.first().copied()?;
    let right_node = non_comment
        .iter()
        .copied()
        .rfind(|c| c.id() != left_node.id() && c.is_named());
    if let Some(right) = right_node {
        infer_elvis_operator(ctx, left_node, right)
    } else {
        let mut sub_ctx = ctx.sub();
        infer_expression_node(&mut sub_ctx, left_node).map(|t| t.make_non_null())
    }
}

pub fn infer_binary_operator(
    ctx: &mut AstEvalContext,
    left_node: Node,
    op: &str,
    right_node: Node,
) -> Option<KotlinType> {
    match op {
        "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" | "&&" | "||" => {
            Some(KotlinType::BOOLEAN)
        }
        ".." | "..<" => Some(infer_range_operator(ctx, left_node, right_node)),
        "?:" => infer_elvis_operator(ctx, left_node, right_node),
        "+" | "-" | "*" | "/" | "%" => infer_arithmetic_operator(ctx, left_node, op, right_node),
        _ => None,
    }
}

fn infer_range_operator(ctx: &mut AstEvalContext, left_node: Node, right_node: Node) -> KotlinType {
    let left_ty = infer_expression_node(&mut ctx.sub(), left_node);
    let right_ty = infer_expression_node(&mut ctx.sub(), right_node);
    if matches!(
        left_ty.as_ref().map(KotlinType::unwrap_nullable_ref),
        Some(KotlinType::Primitive(PrimitiveType::Long))
    ) || matches!(
        right_ty.as_ref().map(KotlinType::unwrap_nullable_ref),
        Some(KotlinType::Primitive(PrimitiveType::Long))
    ) {
        return KotlinType::simple_class("LongRange");
    }
    if matches!(
        left_ty.as_ref().map(KotlinType::unwrap_nullable_ref),
        Some(KotlinType::Primitive(PrimitiveType::Char))
    ) || matches!(
        right_ty.as_ref().map(KotlinType::unwrap_nullable_ref),
        Some(KotlinType::Primitive(PrimitiveType::Char))
    ) {
        return KotlinType::simple_class("CharRange");
    }
    let left_base = left_ty
        .as_ref()
        .and_then(|t| t.base_name().rsplit('.').next())
        .unwrap_or("");
    let right_base = right_ty
        .as_ref()
        .and_then(|t| t.base_name().rsplit('.').next())
        .unwrap_or("");

    if left_base == "ULong" || right_base == "ULong" {
        KotlinType::simple_class("ULongRange")
    } else if left_base == "UInt" || right_base == "UInt" {
        KotlinType::simple_class("UIntRange")
    } else {
        KotlinType::simple_class("IntRange")
    }
}

fn infer_elvis_operator(
    ctx: &mut AstEvalContext,
    left_node: Node,
    right_node: Node,
) -> Option<KotlinType> {
    let mut left_sub = ctx.sub();
    let left_ty = infer_expression_node(&mut left_sub, left_node);
    let mut right_sub = ctx.sub();
    let right_ty = infer_expression_node(&mut right_sub, right_node);

    match (left_ty, right_ty) {
        (Some(l), Some(r)) => {
            let non_null_l = l.make_non_null();
            if non_null_l == r {
                Some(non_null_l)
            } else if let Some(unified) = super::unify::unify_two_types(ctx.index, &non_null_l, &r)
            {
                Some(unified)
            } else {
                Some(non_null_l)
            }
        }
        (Some(l), None) => Some(l.make_non_null()),
        (None, Some(r)) => Some(r),
        _ => None,
    }
}

fn infer_arithmetic_operator(
    ctx: &mut AstEvalContext,
    left_node: Node,
    op: &str,
    right_node: Node,
) -> Option<KotlinType> {
    let mut left_sub = ctx.sub();
    let left_ty = infer_expression_node(&mut left_sub, left_node)?;
    let mut right_sub = ctx.sub();
    let right_ty = infer_expression_node(&mut right_sub, right_node);

    if let Some(res) = infer_builtin_primitive_arithmetic(&left_ty, op, right_ty.as_ref()) {
        return Some(res);
    }

    let left_unwrapped = left_ty.unwrap_nullable_ref();
    if op == "+" && left_unwrapped == &KotlinType::STRING {
        return Some(KotlinType::STRING);
    }

    if matches!(
        left_unwrapped,
        KotlinType::Primitive(PrimitiveType::Int | PrimitiveType::Long)
    ) && matches!(right_node.kind(), "integer_literal")
    {
        Some(KotlinType::INT)
    } else {
        let method_name = match op {
            "+" => "plus",
            "-" => "minus",
            "*" => "times",
            "/" => "div",
            "%" => "rem",
            _ => "plus",
        };
        let right_text = ctx.node_text(right_node)?;
        let member_call = common::concat_str!(method_name, "(", right_text, ")");
        let call_info = super::call_parser::CallInfo {
            callee: method_name,
            explicit_type_args: Vec::new(),
            value_args: vec![right_text],
        };
        let inferred_args = [right_ty];
        super::member::resolve_member_call_info_depth_traced(
            ctx.ctx.sub(),
            &left_ty,
            &member_call,
            &call_info,
            &inferred_args,
        )
    }
}

fn infer_builtin_primitive_arithmetic(
    left_ty: &KotlinType,
    op: &str,
    right_ty: Option<&KotlinType>,
) -> Option<KotlinType> {
    let right = right_ty?;
    let left_unwrapped = left_ty.unwrap_nullable_ref();
    let right_unwrapped = right.unwrap_nullable_ref();
    if op == "+"
        && (left_unwrapped == &KotlinType::STRING || right_unwrapped == &KotlinType::STRING)
    {
        return Some(KotlinType::STRING);
    }

    let left_prim = PrimitiveType::from_name(left_ty.base_name())?;
    let right_prim = PrimitiveType::from_name(right.base_name())?;
    let widened = super::unify::numeric_widening(left_prim, right_prim)?;
    Some(KotlinType::Primitive(widened))
}

pub fn is_binary_kind(kind: &str) -> bool {
    matches!(
        kind,
        "binary_expression"
            | "additive_expression"
            | "multiplicative_expression"
            | "comparison_expression"
            | "equality_expression"
            | "conjunction_expression"
            | "disjunction_expression"
            | "range_expression"
    )
}

pub fn infer_as_cast(
    index: &WorkspaceIndex,
    current_url: &Url,
    node: Node,
    source: &str,
) -> Option<KotlinType> {
    let mut cursor = node.walk();
    let mut is_safe = false;
    let mut past_as = false;
    let mut target_node = None;
    for child in node.children(&mut cursor) {
        if child.kind() == "as" {
            past_as = true;
        } else if child.kind() == "as?" {
            past_as = true;
            is_safe = true;
        } else if past_as && crate::inference::types::ast_type::is_type_node(&child) {
            target_node = Some(child);
            break;
        }
    }
    let target = target_node?;
    let parsed_type =
        crate::inference::types::ast_node_to_kotlin_type(&target, source, &[], false)?;
    let qualified = super::qualification::qualify_type(index, current_url, &parsed_type);
    if is_safe {
        Some(qualified.make_nullable())
    } else {
        Some(qualified)
    }
}
