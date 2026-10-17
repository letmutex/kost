use smol_str::SmolStr;
use tree_sitter::Node;

use super::ast_context::resolve_decl_annotation_type;
use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::ast_narrowing::{extract_if_condition_bindings, extract_when_entry_bindings};
use super::types::KotlinType;
use super::unify::unify_two_types;

/// Infer type for Kotlin control flow expressions (if, when, try)
pub fn infer_control_flow(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    match node.kind() {
        "if_expression" => infer_if_expression(ctx, node),
        "when_expression" => infer_when_expression(ctx, node),
        "try_expression" => infer_try_expression(ctx, node),
        _ => None,
    }
}

/// Check if node kind is a control flow expression
pub fn is_control_expression_kind(kind: &str) -> bool {
    matches!(kind, "if_expression" | "when_expression" | "try_expression")
}

/// Infer if_expression: compute least upper bound of then and else branches
pub fn infer_if_expression(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut cond_node = node.child_by_field_name("condition");
    let mut then_node = node.child_by_field_name("consequence");
    let mut else_node = node.child_by_field_name("alternative");

    if cond_node.is_none() || then_node.is_none() {
        let mut past_else = false;
        let mut past_cond = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "else" {
                past_else = true;
            } else if past_else {
                if child.is_named() && else_node.is_none() {
                    else_node = Some(child);
                }
            } else if !past_cond {
                if child.is_named() {
                    past_cond = true;
                    if cond_node.is_none() {
                        cond_node = Some(child);
                    }
                }
            } else if child.is_named() && then_node.is_none() {
                then_node = Some(child);
            }
        }
    }

    let then_body = then_node?;
    let (then_bindings, else_bindings) = if let Some(cond) = cond_node {
        extract_if_condition_bindings(ctx, cond)
    } else {
        (Vec::new(), Vec::new())
    };

    let mut then_ctx = ctx.sub();
    then_ctx.local_bindings.extend(then_bindings);
    let then_ty = eval_branch_body(&mut then_ctx, then_body);
    drop(then_ctx);

    if let Some(else_branch) = else_node {
        let mut else_ctx = ctx.sub();
        else_ctx.local_bindings.extend(else_bindings);
        let else_ty = eval_branch_body(&mut else_ctx, else_branch);

        match (then_ty, else_ty) {
            (Some(t1), Some(t2)) => unify_two_types(ctx.index, &t1, &t2),
            (Some(t1), None) => Some(t1),
            (None, Some(t2)) => Some(t2),
            (None, None) => None,
        }
    } else {
        Some(KotlinType::UNIT)
    }
}

/// Infer when_expression: compute least upper bound across all when_entry branches
pub fn infer_when_expression(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut subject_name: Option<SmolStr> = None;
    let mut subject_is_val = false;
    let mut subject_binding: Option<(SmolStr, KotlinType)> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "when_subject" {
            let mut sub_cursor = child.walk();
            let mut has_val = false;
            let mut var_decl = None;
            let mut expr_node = None;
            for sub_child in child.children(&mut sub_cursor) {
                if sub_child.kind() == "val" {
                    has_val = true;
                } else if sub_child.kind() == "variable_declaration" {
                    var_decl = Some(sub_child);
                } else if sub_child.is_named() && sub_child.kind() != "annotation" {
                    expr_node = Some(sub_child);
                }
            }
            if has_val
                && let Some(var_decl_node) = var_decl
                && let Some(expr) = expr_node
            {
                if let Some(name) =
                    crate::parser::extract_identifier_text(&var_decl_node, ctx.node_source)
                    && let Some(ty) = infer_expression_node(ctx, expr)
                {
                    subject_name = Some(name.into());
                    subject_is_val = true;
                    subject_binding = Some((name.into(), ty));
                }
            } else if let Some(expr) = expr_node
                && expr.kind() == "simple_identifier"
                && let Some(txt) = ctx.node_text(expr)
            {
                subject_name = Some(txt.into());
            }
            break;
        }
    }

    let mut unified: Option<KotlinType> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "when_entry"
            && let Some(body) = find_when_entry_body(child)
        {
            let mut entry_ctx = ctx.sub();
            if let Some((ref s_name, ref s_ty)) = subject_binding {
                entry_ctx
                    .local_bindings
                    .push((s_name.clone(), s_ty.clone()));
            }
            let bindings = extract_when_entry_bindings(
                &mut entry_ctx,
                child,
                subject_name.as_deref(),
                subject_is_val,
            );
            entry_ctx.local_bindings.extend(bindings);
            if let Some(ty) = eval_branch_body(&mut entry_ctx, body) {
                unified = match unified {
                    None => Some(ty),
                    Some(acc) => unify_two_types(ctx.index, &acc, &ty),
                };
            }
        }
    }
    unified
}

/// Sequentially evaluate branch body statements, registering intermediate declarations
pub fn eval_branch_body(ctx: &mut AstEvalContext, body_node: Node) -> Option<KotlinType> {
    match body_node.kind() {
        "control_structure_body" => {
            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
                if child.is_named() {
                    return eval_branch_body(ctx, child);
                }
            }
            Some(KotlinType::UNIT)
        }
        "statements" => {
            let mut cursor = body_node.walk();
            let mut pending: Option<Node> = None;
            for child in body_node.children(&mut cursor) {
                if !child.is_named()
                    || matches!(
                        child.kind(),
                        "comment" | "line_comment" | "multiline_comment"
                    )
                {
                    continue;
                }
                if let Some(prev) = pending.replace(child)
                    && (prev.kind() == "property_declaration"
                        || prev.kind() == "variable_declaration"
                        || prev.kind() == "destructuring_declaration")
                {
                    register_local_decl_in_branch(ctx, prev);
                }
            }
            if let Some(last) = pending {
                if last.kind() == "property_declaration"
                    || last.kind() == "variable_declaration"
                    || last.kind() == "destructuring_declaration"
                {
                    register_local_decl_in_branch(ctx, last);
                    Some(KotlinType::UNIT)
                } else {
                    infer_expression_node(ctx, last)
                }
            } else {
                Some(KotlinType::UNIT)
            }
        }
        _ => infer_expression_node(ctx, body_node),
    }
}

fn register_local_decl_in_branch(ctx: &mut AstEvalContext, decl: Node) {
    let Some(name) = crate::parser::extract_identifier_text(&decl, ctx.node_source) else {
        return;
    };
    let ty = resolve_decl_annotation_type(ctx, &decl).or_else(|| {
        super::variable::extract_initializer_node(&decl)
            .and_then(|init| infer_expression_node(&mut ctx.sub(), init))
    });
    if let Some(t) = ty {
        ctx.local_bindings.push((name.into(), t));
    }
}

fn find_when_entry_body(entry: Node) -> Option<Node> {
    let mut cursor = entry.walk();
    let mut past_arrow = false;
    let mut after_arrow = None;
    let mut control_body = None;

    for child in entry.children(&mut cursor) {
        if child.kind() == "->" {
            past_arrow = true;
        } else if past_arrow && child.is_named() && after_arrow.is_none() {
            after_arrow = Some(child);
        } else if child.kind() == "control_structure_body" {
            control_body = Some(child);
        }
    }

    control_body.or(after_arrow)
}

/// Infer try_expression: compute least upper bound of try body and all catch blocks
pub fn infer_try_expression(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let mut unified: Option<KotlinType> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "statements" {
            let mut sub = ctx.sub();
            if let Some(ty) = eval_branch_body(&mut sub, child) {
                unified = Some(ty);
            }
        } else if child.kind() == "catch_block"
            && let Some(catch_body) = find_catch_body(child)
        {
            let mut sub = ctx.sub();
            if let Some(ty) = eval_branch_body(&mut sub, catch_body) {
                unified = match unified {
                    None => Some(ty),
                    Some(acc) => unify_two_types(ctx.index, &acc, &ty),
                };
            }
        }
    }
    unified
}

fn find_catch_body(catch_block: Node) -> Option<Node> {
    let mut cursor = catch_block.walk();
    let mut body = None;
    for child in catch_block.children(&mut cursor) {
        if matches!(child.kind(), "statements" | "control_structure_body") {
            body = Some(child);
        }
    }
    body
}
