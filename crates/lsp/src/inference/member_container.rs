use tower_lsp::lsp_types::{SymbolKind, Url};

use super::call::{CallInfo, solve_call_decl_return_type_traced};
use super::context::InferenceContext;
use super::member_ranking::{
    collect_candidate_containers, format_selected_member, record_ranked_candidates,
    sort_by_overload_score,
};
use super::qualification::qualify_type;
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::syntax::signature::extract_return_type_from_signature;

pub fn resolve_container_members(
    mut ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) -> Option<KotlinType> {
    let member_name = call_info.callee;
    let all_containers = collect_candidate_containers(container_type, ctx.index);

    for c in &all_containers {
        if let Some(decls) = ctx.index.container_to_decls.get(c.as_str()) {
            let mut matching: Vec<_> = decls
                .iter()
                .filter(|(_, d)| d.name == member_name)
                .map(|(u, d)| (u.as_ref(), d.as_ref()))
                .collect();
            if matching.is_empty() {
                continue;
            }
            ctx.tracer.record_step(|| {
                crate::concat_str!(
                    "Found ",
                    matching.len().to_string(),
                    " container candidate(s) for `",
                    member_name,
                    "` in `",
                    c,
                    "`"
                )
            });
            sort_by_overload_score(&mut matching, call_info, inferred_arg_types, ctx.index);
            record_ranked_candidates(
                ctx.tracer,
                &matching,
                call_info,
                inferred_arg_types,
                ctx.index,
            );
            for (url, decl) in matching {
                if let Some(inferred) = resolve_single_container_decl(
                    ctx.reborrow(),
                    c,
                    container_type,
                    url,
                    decl,
                    call_info,
                    inferred_arg_types,
                ) {
                    return Some(inferred);
                }
            }
        }
    }
    ctx.tracer.record_step(|| {
        crate::concat_str!(
            "No container members found for `",
            member_name,
            "` on `",
            container_type,
            "`"
        )
    });
    None
}

fn resolve_container_constructor_decl(
    ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    decl: &crate::indexer::DeclarationItem,
    sig: &str,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> Option<KotlinType> {
    let bare = container_type
        .split('<')
        .next()
        .unwrap_or(container_type)
        .trim();
    let full = crate::concat_str!(bare, ".", decl.name);
    if !call_info.explicit_type_args.is_empty() {
        let parameterized =
            crate::concat_str!(full, "<", call_info.explicit_type_args.join(", "), ">");
        return super::types::parse_type_signature(&parameterized);
    }
    let type_params = if let Some(sig_info) = super::signature::SignatureInfo::from_decl(decl) {
        sig_info
            .type_params
            .iter()
            .map(|p| p.name.to_string())
            .collect()
    } else {
        super::generics::extract_generic_type_params(sig)
    };
    if !type_params.is_empty() && !call_info.value_args.is_empty() {
        let template = crate::concat_str!(full, "<", type_params.join(", "), ">");
        let solved = super::call::solve_call_generic_return_type_traced(
            ctx,
            sig,
            &template,
            Some(container_type),
            call_info,
            inferred_arg_types,
        );
        return Some(solved);
    }
    super::types::parse_type_signature(&full)
}

fn resolve_decl_by_signature(
    ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> Option<KotlinType> {
    let sig = decl.signature.as_deref()?;
    if matches!(decl.kind, SymbolKind::CLASS | SymbolKind::CONSTRUCTOR) {
        if !super::qualification::is_declaration_visible(ctx.index, ctx.current_url, url, decl) {
            return None;
        }
        return resolve_container_constructor_decl(
            ctx,
            container_type,
            decl,
            sig,
            call_info,
            inferred_arg_types,
        );
    }
    let target_ret = super::signature::SignatureInfo::from_decl(decl)
        .and_then(|info| info.return_type)
        .or_else(|| extract_return_type_from_signature(sig).and_then(|s| KotlinType::parse(&s)))?;
    let index = ctx.index;
    let solved = solve_call_decl_return_type_traced(
        ctx,
        decl,
        Some(&target_ret),
        Some(container_type),
        call_info,
        inferred_arg_types,
    );
    Some(qualify_type(index, url, &solved))
}

fn resolve_container_decl_from_source(
    index: &WorkspaceIndex,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    depth: usize,
) -> Option<KotlinType> {
    let decl_source = index.get_file_source(url)?;
    if let Some(inferred) = super::function::infer_single_expression_function_return_type(
        index,
        url,
        decl,
        &decl_source,
        depth + 1,
    ) {
        return Some(inferred);
    }
    super::delegate::resolve_property_delegate_from_source(index, url, decl, &decl_source, depth)
}

fn is_decl_applicable(
    index: &WorkspaceIndex,
    current_url: &Url,
    container_type: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
) -> bool {
    if decl.is_private && url != current_url {
        return false;
    }
    let candidate = crate::inference::overload::TypedCandidate::new(decl);
    if let Some(ref decl_rec) = candidate.sig_info.receiver {
        if !super::qualification::is_declaration_visible(index, current_url, url, decl) {
            return false;
        }
        let tps: Vec<smol_str::SmolStr> = candidate
            .sig_info
            .type_params
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let Some(actual_rec) = super::types::parse_type_signature(container_type) else {
            return false;
        };
        if !super::generics::is_receiver_compatible_typed(index, &actual_rec, decl_rec, &tps) {
            return false;
        }
    }
    true
}

fn resolve_single_container_decl(
    mut ctx: InferenceContext<'_, '_, '_>,
    container_str: &str,
    container_type: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) -> Option<KotlinType> {
    if !is_decl_applicable(ctx.index, ctx.current_url, container_type, url, decl) {
        return None;
    }
    let origin = super::trace::format_origin(url);
    if let Some(inferred) = resolve_decl_by_signature(
        ctx.reborrow(),
        container_type,
        url,
        decl,
        call_info,
        Some(inferred_arg_types),
    ) {
        ctx.tracer.record_step(|| {
            format_selected_member(
                decl,
                &crate::concat_str!("-> `", inferred.to_string(), "`"),
                &origin,
            )
        });
        return Some(inferred);
    }
    if let Some(inferred) = resolve_container_decl_from_source(ctx.index, url, decl, ctx.depth) {
        ctx.tracer.record_step(|| {
            format_selected_member(
                decl,
                &crate::concat_str!("(inferred from source: `", inferred.to_string(), "`)"),
                &origin,
            )
        });
        return Some(inferred);
    }
    if matches!(decl.kind, SymbolKind::FUNCTION | SymbolKind::METHOD) {
        ctx.tracer.record_step(|| {
            format_selected_member(
                decl,
                "has no return type or unresolved signature -> defaulted to Unit",
                &origin,
            )
        });
        return Some(KotlinType::UNIT);
    }
    resolve_container_symbol_decl(
        ctx.index,
        ctx.current_url,
        container_str,
        url,
        decl,
        &origin,
        ctx.tracer,
    )
}

fn resolve_container_symbol_decl(
    index: &WorkspaceIndex,
    current_url: &Url,
    container_str: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    origin: &str,
    tracer: &mut Tracer,
) -> Option<KotlinType> {
    if matches!(decl.kind, SymbolKind::CLASS | SymbolKind::OBJECT) {
        if !super::qualification::is_declaration_visible(index, current_url, url, decl) {
            return None;
        }
        let full = crate::concat_str!(container_str, ".", decl.name);
        tracer.record_step(|| {
            format_selected_member(decl, &crate::concat_str!("-> `", &full, "`"), origin)
        });
        return super::types::parse_type_signature(&full);
    }
    if decl.kind == SymbolKind::ENUM_MEMBER {
        tracer.record_step(|| {
            format_selected_member(
                decl,
                &crate::concat_str!("-> `", container_str, "`"),
                origin,
            )
        });
        return super::types::parse_type_signature(container_str);
    }
    None
}
