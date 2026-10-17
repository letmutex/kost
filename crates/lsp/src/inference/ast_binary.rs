use tower_lsp::lsp_types::Url;
use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

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
        ".." | "..<" => Some(KotlinType::simple_class("IntRange")),
        "?:" => infer_elvis_operator(ctx, left_node, right_node),
        "+" | "-" | "*" | "/" | "%" => infer_arithmetic_operator(ctx, left_node, op, right_node),
        _ => None,
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

    let left_base = left_ty.base_name();
    if op == "+" && left_base == "String" {
        return Some(KotlinType::STRING);
    }

    if (left_base == "Int" || left_base == "Long") && matches!(right_node.kind(), "integer_literal")
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
        let member_call = crate::concat_str!(method_name, "(", right_text, ")");
        super::member::resolve_member_type_depth_traced(ctx.ctx.sub(), left_base, &member_call)
    }
}

fn infer_builtin_primitive_arithmetic(
    left_ty: &KotlinType,
    op: &str,
    right_ty: Option<&KotlinType>,
) -> Option<KotlinType> {
    let right = right_ty?;
    if op == "+" && (left_ty.base_name() == "String" || right.base_name() == "String") {
        return Some(KotlinType::STRING);
    }

    let l_rank = numeric_rank(left_ty.base_name())?;
    let r_rank = numeric_rank(right.base_name())?;
    let max_rank = l_rank.max(r_rank);

    let res = match max_rank {
        4 => KotlinType::DOUBLE,
        3 => KotlinType::FLOAT,
        2 => KotlinType::LONG,
        _ => KotlinType::INT,
    };
    Some(res)
}

fn numeric_rank(type_name: &str) -> Option<u8> {
    match type_name {
        "Byte" | "Short" => Some(0),
        "Int" => Some(1),
        "Long" => Some(2),
        "Float" => Some(3),
        "Double" => Some(4),
        _ => None,
    }
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

pub use super::ast_literal::infer_literal;

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
