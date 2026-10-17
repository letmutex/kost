use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
pub use super::lambda_ast::{
    find_enclosing_call_and_lambda, find_lambda_argument_index_in_call,
    find_last_expression_in_lambda, find_param_index_in_lambda, lambda_has_explicit_params,
};
use super::qualification::qualify_type;
use super::signature::{SignatureInfo, SignatureParam};
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};

/// Infer type of a lambda parameter (explicit or implicit `it`) at the given position
pub fn infer_lambda_param_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    param_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let (call_node, lambda_node) = find_enclosing_call_and_lambda(param_node)?;
    let param_idx = find_param_index_in_lambda(param_node, lambda_node);
    let arg_idx = find_lambda_argument_index_in_call(call_node, lambda_node);

    let mut tracer = super::trace::Tracer::Noop;
    let ctx =
        super::InferenceContext::new(index, current_url, pos, source, root, depth, &mut tracer);
    resolve_lambda_param_from_call(ctx, call_node, arg_idx, param_idx)
}

/// Infer type of implicit `it` for a known call and lambda node
pub fn infer_lambda_it_type_for_call(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_node: Node,
    lambda_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    if lambda_has_explicit_params(lambda_node) {
        return None;
    }
    let arg_idx = find_lambda_argument_index_in_call(call_node, lambda_node);

    let mut tracer = super::trace::Tracer::Noop;
    let ctx =
        super::InferenceContext::new(index, current_url, pos, source, root, depth, &mut tracer);
    resolve_lambda_param_from_call(ctx, call_node, arg_idx, 0)
}

/// Infer type of implicit `it` inside a lambda
pub fn infer_lambda_it_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    it_node: Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    let (call_node, lambda_node) = find_enclosing_call_and_lambda(it_node)?;
    infer_lambda_it_type_for_call(
        index,
        current_url,
        call_node,
        lambda_node,
        pos,
        source,
        root,
        depth,
    )
}

struct LambdaCallCtx<'a> {
    index: &'a WorkspaceIndex,
    current_url: &'a Url,
    value_args: &'a [&'a str],
    pos: Position,
    source: &'a str,
    root: Option<Node<'a>>,
    depth: usize,
}

fn resolve_lambda_param_from_call(
    ctx: InferenceContext<'_, '_, '_>,
    call_node: Node,
    arg_idx: usize,
    param_idx: usize,
) -> Option<KotlinType> {
    let callee_node = call_node.child(0)?;
    let callee_raw = callee_node.utf8_text(ctx.source.as_bytes()).ok()?.trim();
    let call_info = super::call::parse_call_expression(callee_raw);
    let callee_clean = call_info.callee.trim();
    let ast_args = super::lambda_ast::extract_call_argument_expressions(call_node, ctx.source);
    let (effective_value_args, effective_arg_idx) = if !call_info.value_args.is_empty() {
        (&call_info.value_args, call_info.value_args.len())
    } else {
        (&ast_args, arg_idx)
    };

    let call_ctx = LambdaCallCtx {
        index: ctx.index,
        current_url: ctx.current_url,
        value_args: effective_value_args,
        pos: ctx.pos,
        source: ctx.source,
        root: ctx.root,
        depth: ctx.depth,
    };

    if let Some((rec_expr, method_name)) = callee_clean.rsplit_once('.') {
        let rec_type = infer_receiver_type_depth(
            ctx.index,
            ctx.current_url,
            rec_expr,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        )?;
        resolve_member_lambda_param(
            &call_ctx,
            &rec_type,
            method_name,
            effective_arg_idx,
            param_idx,
        )
    } else {
        resolve_toplevel_lambda_param(&call_ctx, callee_clean, effective_arg_idx, param_idx)
    }
}

fn resolve_member_lambda_param(
    ctx: &LambdaCallCtx,
    rec_type: &KotlinType,
    method_name: &str,
    arg_idx: usize,
    param_idx: usize,
) -> Option<KotlinType> {
    let clean_container = rec_type.base_name();
    let short_container = clean_container
        .rsplit('.')
        .next()
        .unwrap_or(clean_container);
    let mut all_containers = vec![clean_container.to_string()];
    if short_container != clean_container {
        all_containers.push(short_container.to_string());
    }
    all_containers.extend(ctx.index.get_all_supertypes(clean_container));

    for c in &all_containers {
        if let Some(decls) = ctx.index.container_to_decls.get(c.as_str()) {
            for (decl_url, decl) in decls.iter().filter(|(_, d)| d.name == method_name) {
                if let Some(t) = extract_and_substitute_lambda_param(
                    ctx,
                    decl_url.as_ref(),
                    decl,
                    Some(rec_type),
                    arg_idx,
                    param_idx,
                ) {
                    return Some(t);
                }
            }
        }
    }

    if let Some(decls) = ctx.index.name_to_decls.get(method_name) {
        for (decl_url, decl) in decls {
            if let Some(t) = extract_and_substitute_lambda_param(
                ctx,
                decl_url.as_ref(),
                decl,
                Some(rec_type),
                arg_idx,
                param_idx,
            ) {
                return Some(t);
            }
        }
    }

    None
}

fn resolve_toplevel_lambda_param(
    ctx: &LambdaCallCtx,
    callee_name: &str,
    arg_idx: usize,
    param_idx: usize,
) -> Option<KotlinType> {
    let decls = ctx.index.name_to_decls.get(callee_name)?;
    for (decl_url, decl) in decls {
        if decl.kind == SymbolKind::FUNCTION
            && (!decl.is_private || decl_url.as_ref() == ctx.current_url)
            && let Some(t) = extract_and_substitute_lambda_param(
                ctx,
                decl_url.as_ref(),
                decl,
                None,
                arg_idx,
                param_idx,
            )
        {
            return Some(t);
        }
    }
    None
}

fn extract_and_substitute_lambda_param(
    ctx: &LambdaCallCtx,
    decl_url: &Url,
    decl: &DeclarationItem,
    receiver_type: Option<&KotlinType>,
    arg_idx: usize,
    param_idx: usize,
) -> Option<KotlinType> {
    let sig = decl.signature.as_deref()?;
    let sig_info = SignatureInfo::from_decl(decl);

    let mut system = if let Some(ref info) = sig_info {
        super::solver::ConstraintSystem::from_signature_info(ctx.index, info)
    } else {
        super::solver::ConstraintSystem::from_signature(ctx.index, sig)
    };
    if let Some(rec) = receiver_type {
        system.add_container_type_params_typed(rec);
        if let Some(ref info) = sig_info {
            if let Some(decl_rec) = info.receiver.as_ref() {
                system.add_receiver_constraint_typed(rec, decl_rec);
            }
        } else {
            system.add_receiver_constraint_from_sig(sig, rec);
        }
    }
    for (i, arg_expr) in ctx.value_args.iter().enumerate() {
        if let Some(arg_ty) = infer_receiver_type_depth(
            ctx.index,
            ctx.current_url,
            arg_expr,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        ) {
            if let Some(ref info) = sig_info {
                if let Some(expected) = info.params.get(i).and_then(|p| p.kotlin_type.as_ref()) {
                    system.add_subtype_constraint(&arg_ty, expected);
                }
            } else if let Some(p_str) = crate::signature::extract_param_type_by_index(sig, i)
                && let Some(expected) = system.parse_with_params(&p_str)
            {
                system.add_subtype_constraint(&arg_ty, &expected);
            }
        }
    }

    let target = if let Some(ref info) = sig_info {
        find_matching_functional_param(info, arg_idx, param_idx)
    } else {
        let raw_type = crate::signature::extract_param_type_by_index(sig, arg_idx)?;
        let fn_ty = system.parse_with_params(&raw_type)?;
        if let KotlinType::Function { params, .. } = fn_ty.unwrap_nullable_ref() {
            params.get(param_idx).cloned()
        } else {
            None
        }
    }?;

    let solved = system.solve_and_substitute(&target);
    let resolved = resolve_fallback_type_param(solved, receiver_type);
    Some(qualify_type(ctx.index, decl_url, &resolved))
}

fn resolve_fallback_type_param(
    solved: KotlinType,
    receiver_type: Option<&KotlinType>,
) -> KotlinType {
    if matches!(solved, KotlinType::TypeParam(_))
        && let Some(rec) = receiver_type
        && let KotlinType::Class { args, .. } = rec.unwrap_nullable_ref()
        && args.len() == 1
    {
        return args[0].type_ref.clone();
    }
    solved
}

fn find_matching_functional_param(
    info: &SignatureInfo,
    arg_idx: usize,
    param_idx: usize,
) -> Option<KotlinType> {
    let extract = |p: &SignatureParam| {
        let kt = p.kotlin_type.as_ref()?;
        if let KotlinType::Function { params, .. } = kt.unwrap_nullable_ref() {
            params.get(param_idx).cloned()
        } else {
            None
        }
    };

    if let Some(param) = info.params.get(arg_idx)
        && param
            .kotlin_type
            .as_ref()
            .is_some_and(|kt| kt.is_function())
    {
        return extract(param);
    }

    info.params
        .iter()
        .rev()
        .find(|p| p.kotlin_type.as_ref().is_some_and(|kt| kt.is_function()))
        .and_then(extract)
}

pub use super::lambda_return::infer_lambda_return_type;
