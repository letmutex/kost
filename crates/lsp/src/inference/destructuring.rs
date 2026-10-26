use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::context::InferenceContext;
use super::member::resolve_member_type_kt_depth_traced;
use super::types::KotlinType;
use crate::position::node_to_range;

/// Infer the type of a destructured variable inside a multi_variable_declaration node
pub fn infer_destructured_variable_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    decl_node: &Node,
    clean: &str,
) -> Option<KotlinType> {
    if ctx.depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let multi_var = decl_node
        .parent()
        .filter(|p| p.kind() == "multi_variable_declaration")?;
    let owner = multi_var.parent()?;
    let comp_idx = find_component_index(&multi_var, decl_node)?;

    ctx.tracer.enter_scope("Destructuring", clean);
    let result = match owner.kind() {
        "property_declaration" | "destructuring_declaration" => {
            resolve_property_destructuring(ctx.reborrow(), &owner, comp_idx)
        }
        "for_statement" => resolve_for_loop_destructuring(ctx.reborrow(), &owner, comp_idx),
        "lambda_parameters" => resolve_lambda_destructuring(ctx.reborrow(), multi_var, comp_idx),
        _ => None,
    };

    if let Some(ref kt) = result {
        ctx.tracer.exit_scope(Some(&kt.to_string()));
    } else {
        ctx.tracer.exit_scope(None);
    }
    result
}

fn resolve_property_destructuring(
    mut ctx: InferenceContext<'_, '_, '_>,
    owner: &Node,
    comp_idx: usize,
) -> Option<KotlinType> {
    let rhs_node = crate::syntax::ast_util::extract_initializer_node(owner)?;
    let owner_pos = node_to_range(owner).start;
    let eval_inf_ctx = ctx.sub_with_pos(owner_pos);
    let mut eval_ctx = AstEvalContext::from_ctx(eval_inf_ctx);
    let rhs_type = infer_expression_node(&mut eval_ctx, rhs_node)?;
    resolve_component_on_type(ctx.at_pos(owner_pos), &rhs_type, comp_idx)
}

fn resolve_for_loop_destructuring(
    mut ctx: InferenceContext<'_, '_, '_>,
    for_node: &Node,
    comp_idx: usize,
) -> Option<KotlinType> {
    let iter_node = extract_for_loop_iterable_node(for_node)?;
    let for_pos = node_to_range(for_node).start;
    let eval_inf_ctx = ctx.sub_with_pos(for_pos);
    let mut eval_ctx = AstEvalContext::from_ctx(eval_inf_ctx);
    let iter_type = infer_expression_node(&mut eval_ctx, iter_node)?;

    ctx.tracer
        .record_step(|| "Resolving iterator() operator on for-loop iterable".to_string());
    let iterator_type =
        resolve_member_type_kt_depth_traced(ctx.sub_with_pos(for_pos), &iter_type, "iterator()")?;

    ctx.tracer
        .record_step(|| "Resolving next() operator on iterator".to_string());
    let elem_type =
        resolve_member_type_kt_depth_traced(ctx.sub_with_pos(for_pos), &iterator_type, "next()")?;

    resolve_component_on_type(ctx.at_pos(for_pos), &elem_type, comp_idx)
}

fn resolve_lambda_destructuring(
    ctx: InferenceContext<'_, '_, '_>,
    multi_var: Node,
    comp_idx: usize,
) -> Option<KotlinType> {
    ctx.tracer
        .record_step(|| "Inferring expected lambda parameter type for destructuring".to_string());
    let param_type = super::lambda::infer_lambda_param_type(
        ctx.index,
        ctx.current_url,
        multi_var,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth + 1,
    )?;
    resolve_component_on_type(ctx, &param_type, comp_idx)
}

const COMMON_COMPONENT_CALLS: &[&str] = &[
    "component1()",
    "component2()",
    "component3()",
    "component4()",
    "component5()",
];

fn resolve_component_on_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    receiver_type: &KotlinType,
    comp_idx: usize,
) -> Option<KotlinType> {
    if ctx.depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let fallback_call;
    let comp_call: &str = if comp_idx < COMMON_COMPONENT_CALLS.len() {
        COMMON_COMPONENT_CALLS[comp_idx]
    } else {
        fallback_call = common::concat_str!("component", (comp_idx + 1).to_string(), "()");
        &fallback_call
    };
    ctx.tracer.record_step(|| {
        common::concat_str!(
            "Resolving destructuring operator `",
            comp_call,
            "` on `",
            &receiver_type.to_string(),
            "`"
        )
    });
    resolve_member_type_kt_depth_traced(ctx.sub(), receiver_type, comp_call)
}

fn find_component_index(multi_var: &Node, decl_node: &Node) -> Option<usize> {
    let mut cursor = multi_var.walk();
    let mut idx = 0;
    for child in multi_var.children(&mut cursor) {
        if child.kind() == "variable_declaration" {
            if child.id() == decl_node.id()
                || (child.start_byte() == decl_node.start_byte()
                    && child.end_byte() == decl_node.end_byte())
            {
                return Some(idx);
            }
            idx += 1;
        }
    }
    None
}

fn extract_for_loop_iterable_node<'a>(for_node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = for_node.walk();
    let mut found_in = false;
    for child in for_node.children(&mut cursor) {
        if found_in {
            if child.is_named() && !child.kind().contains("comment") {
                return Some(child);
            }
        } else if child.kind() == "in" {
            found_in = true;
        }
    }
    None
}
