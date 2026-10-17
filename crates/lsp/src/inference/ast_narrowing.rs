use smol_str::SmolStr;
use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::types::KotlinType;

pub type NarrowingBindings = Vec<(SmolStr, KotlinType)>;

/// Extract flow-sensitive bindings for if condition: (consequence_bindings, alternative_bindings)
pub fn extract_if_condition_bindings(
    ctx: &mut AstEvalContext,
    cond: Node,
) -> (NarrowingBindings, NarrowingBindings) {
    match cond.kind() {
        "check_expression" | "is_expression" => extract_is_condition_bindings(ctx, cond),
        "equality_expression" => extract_equality_condition_bindings(ctx, cond),
        _ => (Vec::new(), Vec::new()),
    }
}

fn extract_is_condition_bindings(
    ctx: &AstEvalContext,
    cond: Node,
) -> (NarrowingBindings, NarrowingBindings) {
    let mut cursor = cond.walk();
    let mut target_ident = None;
    let mut is_negated = false;
    let mut type_node = None;

    for child in cond.children(&mut cursor) {
        let k = child.kind();
        if k == "!" || k == "!is" {
            is_negated = true;
        } else if child.is_named() {
            if target_ident.is_none() {
                if k == "simple_identifier" {
                    target_ident = ctx.node_text(child);
                }
            } else if type_node.is_none() && is_type_or_user_type(&child) {
                type_node = Some(child);
            }
        }
    }

    if let (Some(name), Some(ty_node)) = (target_ident, type_node)
        && is_immutable_target(ctx, name, cond)
        && let Some(parsed) =
            super::types::ast_node_to_kotlin_type(&ty_node, ctx.node_source, &[], false)
    {
        let qualified = super::qualification::qualify_type(ctx.index, ctx.current_url, &parsed);
        if is_negated {
            return (Vec::new(), vec![(name.into(), qualified)]);
        } else {
            return (vec![(name.into(), qualified)], Vec::new());
        }
    }
    (Vec::new(), Vec::new())
}

fn extract_equality_condition_bindings(
    ctx: &mut AstEvalContext,
    cond: Node,
) -> (NarrowingBindings, NarrowingBindings) {
    let mut cursor = cond.walk();
    let mut left = None;
    let mut op = None;
    let mut right = None;

    for child in cond.children(&mut cursor) {
        if child.is_named() {
            if left.is_none() {
                left = Some(child);
            } else if right.is_none() {
                right = Some(child);
            }
        } else {
            let k = child.kind();
            if matches!(k, "==" | "===" | "!=" | "!==") {
                op = Some(k);
            }
        }
    }

    let (Some(left_expr), Some(op_str), Some(right_expr)) = (left, op, right) else {
        return (Vec::new(), Vec::new());
    };

    let target_node = if is_null_node(right_expr) && left_expr.kind() == "simple_identifier" {
        Some(left_expr)
    } else if is_null_node(left_expr) && right_expr.kind() == "simple_identifier" {
        Some(right_expr)
    } else {
        None
    };

    let Some(target) = target_node else {
        return (Vec::new(), Vec::new());
    };
    let Some(name) = ctx.node_text(target) else {
        return (Vec::new(), Vec::new());
    };

    if !is_immutable_target(ctx, name, cond) {
        return (Vec::new(), Vec::new());
    }

    let ambient_ty = infer_expression_node(&mut ctx.sub(), target);
    let is_not_equal = matches!(op_str, "!=" | "!==");

    if let Some(ty) = ambient_ty {
        let non_null = ty.unwrap_nullable_ref().clone();
        let null_ty = KotlinType::NOTHING.make_nullable();

        if is_not_equal {
            (vec![(name.into(), non_null)], vec![(name.into(), null_ty)])
        } else {
            (vec![(name.into(), null_ty)], vec![(name.into(), non_null)])
        }
    } else {
        (Vec::new(), Vec::new())
    }
}

/// Extract flow-sensitive bindings for when_entry condition
pub fn extract_when_entry_bindings(
    ctx: &mut AstEvalContext,
    entry: Node,
    subject_name: Option<&str>,
    subject_is_val: bool,
) -> Vec<(SmolStr, KotlinType)> {
    let mut condition_nodes = Vec::new();
    let mut cursor = entry.walk();
    for child in entry.children(&mut cursor) {
        if child.kind() == "->" {
            break;
        }
        if child.kind() == "when_condition" {
            condition_nodes.push(child);
        }
    }

    if condition_nodes.is_empty() {
        return Vec::new();
    }

    if let Some(name) = subject_name {
        if !subject_is_val && !is_immutable_target(ctx, name, entry) {
            return Vec::new();
        }

        let mut condition_types = Vec::with_capacity(condition_nodes.len());
        for cond in condition_nodes {
            let mut cond_type = None;
            let mut cond_c = cond.walk();
            for c in cond.children(&mut cond_c) {
                if c.kind() == "type_test" {
                    if let Some((_, ty)) = extract_type_test_binding(ctx, c, name) {
                        cond_type = Some(ty);
                        break;
                    }
                } else if is_null_node(c) {
                    cond_type = Some(KotlinType::NOTHING.make_nullable());
                    break;
                }
            }
            if let Some(ty) = cond_type {
                condition_types.push(ty);
            } else {
                return Vec::new();
            }
        }

        let mut folded: Option<KotlinType> = None;
        for ty in condition_types {
            folded = match folded {
                None => Some(ty),
                Some(acc) => super::unify::unify_two_types(ctx.index, &acc, &ty),
            };
        }
        if let Some(lub) = folded {
            vec![(name.into(), lub)]
        } else {
            Vec::new()
        }
    } else if condition_nodes.len() == 1 {
        let mut bindings = Vec::new();
        let mut cc = condition_nodes[0].walk();
        for c in condition_nodes[0].children(&mut cc) {
            if c.is_named() {
                let (pos_bindings, _) = extract_if_condition_bindings(ctx, c);
                bindings.extend(pos_bindings);
            }
        }
        bindings
    } else {
        Vec::new()
    }
}

fn extract_type_test_binding(
    ctx: &AstEvalContext,
    type_test: Node,
    name: &str,
) -> Option<(SmolStr, KotlinType)> {
    let mut cursor = type_test.walk();
    let mut is_negated = false;
    let mut type_node = None;

    for child in type_test.children(&mut cursor) {
        if child.kind() == "!" || child.kind() == "!is" {
            is_negated = true;
        } else if child.is_named() && is_type_or_user_type(&child) {
            type_node = Some(child);
        }
    }

    if !is_negated
        && let Some(tn) = type_node
        && let Some(parsed) =
            super::types::ast_node_to_kotlin_type(&tn, ctx.node_source, &[], false)
    {
        let qualified = super::qualification::qualify_type(ctx.index, ctx.current_url, &parsed);
        Some((name.into(), qualified))
    } else {
        None
    }
}

pub fn is_null_node(node: Node) -> bool {
    matches!(node.kind(), "null" | "null_literal")
}

pub fn is_type_or_user_type(node: &Node) -> bool {
    super::types::ast_type::is_type_node(node)
}

pub fn is_immutable_target(ctx: &AstEvalContext, name: &str, node: Node) -> bool {
    let mut current = Some(node);
    while let Some(curr) = current {
        if let Some(found) = crate::syntax::scope::find_local_variable_or_param_before(
            curr,
            name,
            ctx.node_source,
            node.start_byte(),
        ) {
            let decl_kind = found.kind();
            if decl_kind == "parameter" {
                return true;
            }
            if decl_kind == "class_parameter" {
                return !has_var_modifier(&found);
            }
            if decl_kind == "property_declaration" || decl_kind == "variable_declaration" {
                let mut decl_node = if decl_kind == "variable_declaration" {
                    found.parent().unwrap_or(found)
                } else {
                    found
                };
                if decl_node.kind() == "multi_variable_declaration"
                    && let Some(parent) = decl_node.parent()
                {
                    decl_node = parent;
                }
                let mut cursor = decl_node.walk();
                let has_child_getter_or_delegate = decl_node
                    .children(&mut cursor)
                    .any(|c| matches!(c.kind(), "getter" | "property_delegate"));
                let has_sibling_getter = decl_node
                    .next_named_sibling()
                    .is_some_and(|s| s.kind() == "getter");
                let is_mutable_or_dynamic = has_var_modifier(&decl_node)
                    || has_child_getter_or_delegate
                    || has_sibling_getter;
                return !is_mutable_or_dynamic;
            }
            return true;
        }
        current = curr.parent();
    }
    false
}

fn has_var_modifier(node: &Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| {
        c.kind() == "var"
            || (c.kind() == "binding_pattern_kind" && c.child(0).is_some_and(|k| k.kind() == "var"))
    })
}
