use tower_lsp::lsp_types::{SymbolKind, Url};

use super::context::InferenceContext;
pub use super::lambda_call_candidate::{
    solve_decl_higher_order_call, solve_sig_info_higher_order_call,
};
use super::lambda_call_extension::{
    collect_ranked_extension_candidates, is_callable_function_decl,
};
use super::lambda_call_solve::{
    extract_explicit_type_args_from_call, infer_builder_lambda_receiver,
};
use super::lambda_return::{
    extract_callee_from_ast, find_call_node, find_call_with_lambda, find_lambda_node_and_arg_idx,
    infer_call_lambda_return_type,
};
use super::overload::TypedCandidate;
use super::qualification::is_declaration_visible;
use super::signature::SignatureInfo;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::parse_source;

/// Check if expression is a call containing a lambda (e.g. `remember { ... }` or `map { ... }`)
pub fn is_call_with_lambda(expr: &str) -> bool {
    let clean = expr.trim();
    clean.ends_with('}') && clean.contains('{')
}

/// Resolve return type of a higher-order function call with a lambda
pub fn resolve_higher_order_call(
    mut ctx: InferenceContext<'_, '_, '_>,
    receiver_type: Option<&KotlinType>,
    call_expr: &str,
) -> Option<KotlinType> {
    if ctx.depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let call_tree = parse_source(call_expr)?;
    let call_root = call_tree.root_node();
    let (call_node, lambda_node, arg_idx) = find_call_with_lambda(call_root).or_else(|| {
        let call_node = find_call_node(call_root)?;
        let (lambda_node, arg_idx) = find_lambda_node_and_arg_idx(call_root)?;
        Some((call_node, lambda_node, arg_idx))
    })?;
    let callee = extract_callee_from_ast(call_node, call_expr)?;
    let explicit_type_args =
        extract_explicit_type_args_from_call(call_node, call_expr, ctx.index, ctx.current_url);
    let value_args = super::lambda_ast::extract_call_argument_expressions(call_node, call_expr);
    let is_trailing = crate::syntax::ast_call_view::AstCallView::from_node(call_node)
        .and_then(|view| view.trailing_lambda)
        .is_some_and(|tl| {
            tl.start_byte() <= lambda_node.start_byte() && lambda_node.end_byte() <= tl.end_byte()
        })
        || arg_idx >= value_args.len();

    let expected_fn = super::scope_lambda::resolve_lambda_signature_for_receiver(
        ctx.reborrow(),
        receiver_type,
        callee,
        &explicit_type_args,
        &value_args,
        arg_idx,
        is_trailing,
    );

    let lambda_ret = if let Some(KotlinType::Function {
        receiver: fn_rec,
        return_type,
        ..
    }) = expected_fn.as_ref()
        && return_type.is_unit()
    {
        infer_builder_lambda_receiver(ctx.reborrow(), lambda_node, call_expr, fn_rec.as_deref())
            .or(Some(KotlinType::UNIT))
    } else {
        infer_call_lambda_return_type(
            ctx.reborrow(),
            lambda_node,
            call_expr,
            call_root,
            expected_fn.as_ref(),
        )
    };

    dispatch_higher_order_call(
        &ctx,
        receiver_type,
        callee,
        &explicit_type_args,
        lambda_ret.as_ref(),
        arg_idx,
        is_trailing,
    )
}

fn dispatch_higher_order_call(
    ctx: &InferenceContext<'_, '_, '_>,
    receiver_type: Option<&KotlinType>,
    callee: &str,
    explicit_type_args: &[KotlinType],
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    if let Some(rec) = receiver_type {
        return resolve_member_higher_order(
            ctx.index,
            ctx.current_url,
            rec,
            callee,
            explicit_type_args,
            lambda_ret,
            arg_idx,
            is_trailing,
        );
    }
    if let Some(res) = resolve_toplevel_higher_order(
        ctx.index,
        ctx.current_url,
        callee,
        explicit_type_args,
        lambda_ret,
        arg_idx,
        is_trailing,
    ) {
        return Some(res);
    }
    for implicit_rec in super::scope_stack::collect_implicit_receivers(
        ctx.index,
        ctx.current_url,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    ) {
        if let Some(res) = resolve_member_higher_order(
            ctx.index,
            ctx.current_url,
            &implicit_rec,
            callee,
            explicit_type_args,
            lambda_ret,
            arg_idx,
            is_trailing,
        ) {
            return Some(res);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn resolve_member_higher_order(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver_type: &KotlinType,
    callee: &str,
    explicit_type_args: &[KotlinType],
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    let clean_container = receiver_type.base_name();
    let candidate_containers =
        super::member_ranking::collect_candidate_containers(clean_container, index);

    for container_name in &candidate_containers {
        if let Some(decls) = index.container_to_decls.get(container_name.as_str()) {
            for (url, decl) in decls.iter().filter(|(_, d)| d.name == callee) {
                if !is_callable_function_decl(decl)
                    || (decl.is_private && url.as_ref() != current_url)
                {
                    continue;
                }
                let candidate = TypedCandidate::new(decl);
                if candidate.sig_info.receiver.is_some() {
                    continue;
                }
                if let Some(res) = solve_decl_higher_order_call(
                    index,
                    decl,
                    url.as_ref(),
                    &candidate.sig_info,
                    Some(receiver_type),
                    explicit_type_args,
                    lambda_ret,
                    arg_idx,
                    is_trailing,
                ) {
                    return Some(res);
                }
            }
        }
    }

    for candidate in collect_ranked_extension_candidates(index, current_url, receiver_type, callee)
    {
        if let Some(res) = solve_decl_higher_order_call(
            index,
            candidate.decl,
            candidate.url,
            &candidate.sig_info,
            Some(receiver_type),
            explicit_type_args,
            lambda_ret,
            arg_idx,
            is_trailing,
        ) {
            return Some(res);
        }
    }

    if let Some(builtin_sig) = super::scope_lambda::get_builtin_scope_function(callee)
        && let Some(sig_info) = SignatureInfo::parse(builtin_sig)
    {
        return solve_sig_info_higher_order_call(
            index,
            current_url,
            &sig_info,
            Some(receiver_type),
            explicit_type_args,
            lambda_ret,
            arg_idx,
            false,
            is_trailing,
        );
    }
    None
}

fn resolve_toplevel_higher_order(
    index: &WorkspaceIndex,
    current_url: &Url,
    callee: &str,
    explicit_type_args: &[KotlinType],
    lambda_ret: Option<&KotlinType>,
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    if let Some(decls) = index.name_to_decls.get(callee) {
        for (url, decl) in decls {
            if decl.container_name.is_none()
                && decl.kind == SymbolKind::FUNCTION
                && is_declaration_visible(index, current_url, url.as_ref(), decl)
            {
                let candidate = TypedCandidate::new(decl);
                if candidate.sig_info.receiver.is_none()
                    && let Some(res) = solve_decl_higher_order_call(
                        index,
                        decl,
                        url.as_ref(),
                        &candidate.sig_info,
                        None,
                        explicit_type_args,
                        lambda_ret,
                        arg_idx,
                        is_trailing,
                    )
                {
                    return Some(res);
                }
            }
        }
    }

    if let Some(builtin_sig) = super::scope_lambda::get_builtin_scope_function(callee)
        && let Some(sig_info) = SignatureInfo::parse(builtin_sig)
        && sig_info.receiver.is_none()
    {
        return solve_sig_info_higher_order_call(
            index,
            current_url,
            &sig_info,
            None,
            explicit_type_args,
            lambda_ret,
            arg_idx,
            false,
            is_trailing,
        );
    }
    None
}
