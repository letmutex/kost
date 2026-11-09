use std::collections::HashSet;
use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::call::solve_call_decl_info_return_type_traced;
use super::call_parser::{CallInfo, parse_call_expression};
use super::constructor::find_constructor_return_type;
use super::context::InferenceContext;
use super::overload::{TypedCandidate, score_typed_candidate};
use super::qualification::{is_container_member_imported, is_declaration_visible, qualify_type};
use super::scope_function::collect_visible_containers_url_depth;
pub use super::scope_stack::infer_single_expression_function_return_type;
use super::scope_stack::infer_single_expression_with_root;
use super::signature::SignatureInfo;
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};
use crate::syntax::ast_call_view::parse_call_arg;
use crate::syntax::signature::{
    extract_receiver_type_from_signature, extract_return_type_from_signature,
};

/// Infer the return type of a function given its name and calling context
pub fn infer_function_return_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    fn_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    find_function_return_type_depth(index, current_url, fn_name, pos, source, root, 0)
}

/// Find function return type from uppercase constructor heuristic, declaration index, or definition lookup
pub fn find_function_return_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    fn_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    find_function_return_type_depth(index, current_url, fn_name, pos, source, root, 0)
}

/// Find function return type with recursion depth guard
pub fn find_function_return_type_depth(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_expr: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let call_info = parse_call_expression(call_expr);
    let fn_name = call_info.callee;

    let mut tracer = Tracer::Noop;
    let mut ctx = InferenceContext::new(index, current_url, pos, source, root, depth, &mut tracer);

    if let Some(ctor_type) = find_constructor_return_type(ctx.reborrow(), fn_name, &call_info) {
        return Some(ctor_type);
    }

    if let Some(inferred) = find_top_level_function_return_type(ctx.reborrow(), fn_name, &call_info)
    {
        return Some(inferred);
    }

    if let Some(inferred) =
        find_definition_function_return_type(ctx.reborrow(), fn_name, &call_info)
    {
        return Some(inferred);
    }

    super::scope_stack::find_local_function_return_type(ctx, fn_name)
}

fn is_callable_top_level_or_in_scope(
    ctx: &InferenceContext<'_, '_, '_>,
    url: &Url,
    decl: &DeclarationItem,
    visible_containers: &mut Option<HashSet<String>>,
) -> bool {
    if !matches!(decl.kind, SymbolKind::FUNCTION | SymbolKind::METHOD) {
        return false;
    }
    if decl.is_private && url != ctx.current_url {
        return false;
    }
    let Some(ref container) = decl.container_name else {
        return is_declaration_visible(ctx.index, ctx.current_url, url, decl);
    };
    let short_container = container.rsplit('.').next().unwrap_or(container.as_str());
    if is_container_member_imported(ctx.index, ctx.current_url, short_container, &decl.name) {
        return true;
    }
    let containers = visible_containers.get_or_insert_with(|| {
        collect_visible_containers_url_depth(
            ctx.index,
            ctx.current_url,
            ctx.root,
            ctx.pos,
            ctx.source,
            ctx.depth,
        )
    });
    if !containers.contains(container.as_str()) && !containers.contains(short_container) {
        return false;
    }
    let is_extension = decl
        .signature
        .as_deref()
        .and_then(extract_receiver_type_from_signature)
        .is_some();
    !is_extension || is_declaration_visible(ctx.index, ctx.current_url, url, decl)
}

fn rank_top_level_candidates<'a>(
    candidates: &[(
        &'a std::sync::Arc<Url>,
        &'a std::sync::Arc<DeclarationItem>,
        TypedCandidate<'a>,
    )],
    parsed_args: &[crate::syntax::ast_call_view::ParsedCallArg],
    inferred_arg_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) -> Vec<usize> {
    let mut scored: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, (url, decl, typed))| {
            let is_src = crate::indexer::is_source_url(url);
            let score =
                score_typed_candidate(typed, is_src, parsed_args, inferred_arg_types, index)?;
            let is_top_level = decl.container_name.is_none() && typed.sig_info.receiver.is_none();
            Some((idx, (score, is_top_level)))
        })
        .collect();
    scored.sort_by_key(|a| std::cmp::Reverse(a.1));
    if scored.is_empty() {
        (0..candidates.len()).collect()
    } else {
        scored.into_iter().map(|(idx, _)| idx).collect()
    }
}

fn find_top_level_function_return_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    fn_name: &str,
    call_info: &CallInfo,
) -> Option<KotlinType> {
    let decls = ctx.index.name_to_decls.get(fn_name)?;
    let mut visible_containers: Option<HashSet<String>> = None;
    let candidates: Vec<_> = decls
        .iter()
        .filter(|(url, decl)| {
            is_callable_top_level_or_in_scope(&ctx, url.as_ref(), decl, &mut visible_containers)
        })
        .map(|(url, decl)| (url, decl, TypedCandidate::new(decl.as_ref())))
        .collect();
    if candidates.is_empty() {
        return None;
    }

    let parsed_args: Vec<_> = call_info
        .value_args
        .iter()
        .map(|arg| parse_call_arg(arg))
        .collect();
    let inferred_arg_types = super::member_ranking::infer_call_argument_types(
        ctx.index,
        ctx.current_url,
        call_info,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    );

    for idx in rank_top_level_candidates(&candidates, &parsed_args, &inferred_arg_types, ctx.index)
    {
        let (url, decl, ref typed) = candidates[idx];
        if let Some(inferred) = resolve_candidate_decl_return_type(
            ctx.sub(),
            url.as_ref(),
            decl.as_ref(),
            Some(&typed.sig_info),
            call_info,
            Some(&inferred_arg_types),
        ) {
            return Some(inferred);
        }
    }
    None
}

fn find_definition_function_return_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    fn_name: &str,
    call_info: &CallInfo,
) -> Option<KotlinType> {
    let cf = ctx.index.files.get(ctx.current_url)?;
    if let Some(decl) = cf
        .declarations
        .iter()
        .find(|d| d.name == fn_name && matches!(d.kind, SymbolKind::FUNCTION | SymbolKind::METHOD))
    {
        let current_url = ctx.current_url.clone();
        return resolve_candidate_decl_return_type(ctx, &current_url, decl, None, call_info, None);
    }
    let imp = cf
        .imports
        .iter()
        .find(|i| i.alias.as_deref() == Some(fn_name))?;
    let real_name = imp.path.rsplit('.').next().unwrap_or(fn_name);
    for (url, decl) in ctx.index.name_to_decls.get(real_name)? {
        if matches!(decl.kind, SymbolKind::FUNCTION | SymbolKind::METHOD)
            && let Some(inferred) = resolve_candidate_decl_return_type(
                ctx.sub(),
                url.as_ref(),
                decl.as_ref(),
                None,
                call_info,
                None,
            )
        {
            return Some(inferred);
        }
    }
    None
}

fn resolve_candidate_decl_return_type(
    ctx: InferenceContext<'_, '_, '_>,
    decl_url: &Url,
    decl: &DeclarationItem,
    preparsed_sig: Option<&SignatureInfo>,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> Option<KotlinType> {
    let owned_sig = preparsed_sig
        .is_none()
        .then(|| SignatureInfo::from_decl(decl))
        .flatten();
    let sig_info = preparsed_sig.or(owned_sig.as_ref());

    if let Some(sig) = decl.signature.as_deref()
        && let Some(ret) = sig_info
            .and_then(|info| info.return_type.clone())
            .or_else(|| extract_return_type_from_signature(sig).and_then(|s| KotlinType::parse(&s)))
            .or_else(|| {
                super::lambda_call_candidate::infer_decl_single_expression_return_type(
                    ctx.index,
                    decl_url,
                    decl,
                    sig_info,
                    ctx.depth + 1,
                )
            })
    {
        let index = ctx.index;
        let solved = solve_call_decl_info_return_type_traced(
            ctx,
            decl,
            sig_info,
            Some(&ret),
            None,
            call_info,
            inferred_arg_types,
        );
        return Some(qualify_type(index, decl_url, &solved));
    }

    let in_memory = (decl_url == ctx.current_url)
        .then(|| {
            infer_single_expression_with_root(
                ctx.index,
                decl_url,
                decl,
                ctx.source,
                ctx.root,
                ctx.depth + 1,
            )
        })
        .flatten();
    let single_expr = in_memory.or_else(|| {
        crate::indexer::is_source_url(decl_url)
            .then(|| ctx.index.get_file_source(decl_url))
            .flatten()
            .and_then(|src| {
                infer_single_expression_function_return_type(
                    ctx.index,
                    decl_url,
                    decl,
                    &src,
                    ctx.depth + 1,
                )
            })
    });
    single_expr.or_else(|| (decl.kind == SymbolKind::FUNCTION).then_some(KotlinType::UNIT))
}
