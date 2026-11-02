use tower_lsp::lsp_types::Url;
use tree_sitter::Node;

use super::context::InferenceContext;
use super::lambda_call_extension::check_type_param_bounds_satisfaction;
use super::lambda_return::find_matching_lambda_in_ast;
use super::qualification::qualify_type;
use super::signature::{SignatureInfo, SignatureParam, are_unpassed_parameters_compatible};
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::parse_source;

fn find_first_builder_call<'a>(node: Node<'a>, source: &'a str) -> Option<(&'a str, Node<'a>)> {
    if node.kind() == "call_expression"
        && let Some(view) = crate::syntax::ast_call_view::AstCallView::from_node(node)
        && view.value_arguments.len() == 1
        && let Some(callee) = view.callee_text(source)
        && !callee.contains('.')
        && let Some(arg_expr) =
            crate::syntax::ast_call_view::AstCallView::argument_expression(view.value_arguments[0])
    {
        return Some((callee, arg_expr));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "lambda_literal"
            && let Some(found) = find_first_builder_call(child, source)
        {
            return Some(found);
        }
    }
    None
}

pub fn infer_builder_lambda_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    lambda_node: Node,
    call_expr: &str,
    fn_rec: Option<&KotlinType>,
) -> Option<KotlinType> {
    let rec_ty = fn_rec?;
    if rec_ty.type_arguments().is_empty() {
        return None;
    }
    let parsed_tree = ctx
        .root
        .is_none()
        .then(|| parse_source(ctx.source))
        .flatten();
    let outer_root = ctx
        .root
        .or_else(|| parsed_tree.as_ref().map(|t| t.root_node()))?;
    let lambda_text = lambda_node.utf8_text(call_expr.as_bytes()).ok()?;
    let matched_lambda = find_matching_lambda_in_ast(outer_root, ctx.source, lambda_text, ctx.pos)?;
    let (method_name, arg_node) = find_first_builder_call(matched_lambda, ctx.source)?;

    let container_base = rec_ty.base_name();
    let short_container = container_base.rsplit('.').next().unwrap_or(container_base);
    let decls = ctx
        .index
        .container_to_decls
        .get(container_base)
        .or_else(|| ctx.index.container_to_decls.get(short_container))?;
    if !decls.iter().any(|(_, d)| d.name == method_name) {
        return None;
    }

    let arg_pos = crate::position::point_to_position(arg_node.start_position());
    let mut tracer = super::trace::Tracer::Noop;
    let inf_ctx = InferenceContext::new(
        ctx.index,
        ctx.current_url,
        arg_pos,
        ctx.source,
        Some(outer_root),
        ctx.depth + 1,
        &mut tracer,
    );
    let mut eval_ctx = super::ast_eval::AstEvalContext::from_ctx(inf_ctx);
    let arg_ty = super::ast_eval::infer_expression_node(&mut eval_ctx, arg_node)?;

    let instantiated_rec = KotlinType::class(
        container_base,
        vec![super::types::TypeArg::invariant(arg_ty)],
    );
    Some(KotlinType::function(
        Some(instantiated_rec),
        Vec::new(),
        KotlinType::UNIT,
        false,
    ))
}

pub fn map_trailing_lambda_param_index(
    index: &WorkspaceIndex,
    params: &[SignatureParam],
    preceding_args: usize,
) -> usize {
    if params.is_empty() {
        return preceding_args;
    }
    let is_functional = |param: &SignatureParam| {
        param
            .kotlin_type
            .as_ref()
            .is_some_and(|kt| super::types::is_functional_or_sam(kt, index))
    };
    params
        .iter()
        .rposition(&is_functional)
        .unwrap_or(preceding_args)
}

pub fn solve_sig_info_higher_order_call(
    index: &WorkspaceIndex,
    decl_url: &Url,
    sig_info: &SignatureInfo,
    receiver_type: Option<&KotlinType>,
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    has_container: bool,
    is_trailing: bool,
) -> Option<KotlinType> {
    if receiver_type.is_none() && sig_info.receiver.is_some() {
        return None;
    }
    let mut system = ConstraintSystem::from_signature_info(index, sig_info);
    if let Some(rec) = receiver_type {
        if let Some(ref decl_rec) = sig_info.receiver {
            if !system.add_receiver_constraint_typed(rec, decl_rec) {
                return None;
            }
        } else if has_container {
            system.add_container_type_params_typed(rec);
        } else {
            return None;
        }
    }
    let params = &sig_info.params;
    let target_idx = if is_trailing {
        map_trailing_lambda_param_index(index, params, arg_idx)
    } else {
        arg_idx
    };
    if !are_unpassed_parameters_compatible(params, target_idx, arg_idx) {
        return None;
    }
    let target_param = params.get(target_idx)?;
    let target_ty = target_param
        .kotlin_type
        .clone()
        .or_else(|| system.parse_with_params(&target_param.type_name))?;
    if !super::types::is_functional_or_sam(target_ty.unwrap_nullable_ref(), index) {
        return None;
    }
    if let Some(lambda_return_type) = lambda_ret
        && !system.add_lambda_return_constraint_typed(&target_ty, lambda_return_type)
    {
        return None;
    }
    if let (
        Some(KotlinType::Function {
            receiver: Some(actual_rec),
            ..
        }),
        KotlinType::Function {
            receiver: Some(decl_fn_rec),
            ..
        },
    ) = (lambda_ret, target_ty.unwrap_nullable_ref())
    {
        system.add_subtype_constraint(actual_rec, decl_fn_rec);
    }
    if !check_type_param_bounds_satisfaction(index, &sig_info.type_params, &system.bindings) {
        return None;
    }
    let target = sig_info.return_type.clone().unwrap_or(KotlinType::UNIT);
    let solved = system.solve_and_substitute(&target);
    Some(qualify_type(index, decl_url, &solved))
}

pub fn solve_decl_higher_order_call(
    index: &WorkspaceIndex,
    decl: &crate::indexer::DeclarationItem,
    decl_url: &Url,
    sig_info: &SignatureInfo,
    receiver_type: Option<&KotlinType>,
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    solve_sig_info_higher_order_call(
        index,
        decl_url,
        sig_info,
        receiver_type,
        lambda_ret,
        arg_idx,
        decl.container_name.is_some(),
        is_trailing,
    )
}
