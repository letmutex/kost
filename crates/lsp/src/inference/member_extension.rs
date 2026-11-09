use tower_lsp::lsp_types::{SymbolKind, Url};

use super::call::solve_call_decl_return_type_traced;
use super::call_parser::CallInfo;
use super::context::InferenceContext;
use super::member_ranking::{record_ranked_candidates, sort_by_overload_score};
use super::qualification::qualify_type;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::syntax::signature::extract_return_type_from_signature;

pub fn resolve_extension_members(
    mut ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) -> Option<KotlinType> {
    let decls = match ctx.index.name_to_decls.get(call_info.callee) {
        Some(d) => d,
        None => {
            ctx.tracer.record_step(|| {
                common::concat_str!(
                    "No extension candidates found named `",
                    call_info.callee,
                    "`"
                )
            });
            return None;
        }
    };
    let actual_rec = super::types::parse_type_signature(container_type)?;
    let rec_supertypes = ctx.index.get_all_supertypes(actual_rec.base_name());
    let mut matching: Vec<_> = decls
        .iter()
        .filter(|(u, d)| {
            super::lambda_call_extension::is_extension_container_plausible(
                ctx.index,
                &actual_rec,
                &rec_supertypes,
                d.as_ref(),
            ) && d.signature.as_deref().is_some_and(|sig| {
                is_valid_extension_receiver(
                    ctx.index,
                    ctx.current_url,
                    container_type,
                    u.as_ref(),
                    d.as_ref(),
                    sig,
                    ctx.tracer,
                )
            })
        })
        .map(|(u, d)| (u.as_ref(), d.as_ref()))
        .collect();
    sort_by_overload_score(&mut matching, call_info, inferred_arg_types, ctx.index);
    ctx.tracer.record_step(|| {
        common::concat_str!(
            "Found ",
            matching.len().to_string(),
            " extension candidate(s) for `",
            call_info.callee,
            "`"
        )
    });
    record_ranked_candidates(
        ctx.tracer,
        &matching,
        call_info,
        inferred_arg_types,
        ctx.index,
    );
    for (url, decl) in matching {
        if let Some(inferred) = try_resolve_extension_candidate(
            ctx.reborrow(),
            container_type,
            url,
            decl,
            call_info,
            inferred_arg_types,
        ) {
            return Some(inferred);
        }
    }
    None
}

fn is_valid_extension_receiver(
    index: &WorkspaceIndex,
    current_url: &Url,
    container_type: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    _sig: &str,
    tracer: &mut super::trace::Tracer,
) -> bool {
    if !super::qualification::is_declaration_visible(index, current_url, url, decl) {
        return false;
    }
    let candidate = crate::inference::overload::TypedCandidate::new(decl);
    let Some(ref decl_rec) = candidate.sig_info.receiver else {
        return false;
    };
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
        tracer.record_step(|| {
            common::concat_str!(
                "Skipped candidate `",
                decl.name.as_str(),
                "`: receiver `",
                decl_rec.to_string(),
                "` incompatible with `",
                container_type,
                "`"
            )
        });
        return false;
    }
    true
}

fn resolve_explicit_return_candidate(
    mut ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    ret: &KotlinType,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) -> KotlinType {
    let origin = super::trace::format_origin(url);
    let solved = solve_call_decl_return_type_traced(
        ctx.reborrow(),
        decl,
        Some(ret),
        Some(container_type),
        call_info,
        Some(inferred_arg_types),
    );
    let qualified = qualify_type(ctx.index, url, &solved);
    let sig = decl.signature.as_deref().unwrap_or("");
    ctx.tracer.record_step(|| {
        common::concat_str!(
            "Selected candidate `",
            decl.name.as_str(),
            "` (`",
            sig,
            "`) -> `",
            qualified.to_string(),
            "` ",
            origin
        )
    });
    qualified
}

fn extract_or_infer_extension_return_type(
    index: &WorkspaceIndex,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    sig: &str,
    depth: usize,
) -> Option<KotlinType> {
    let sig_info = super::signature::SignatureInfo::from_decl(decl);
    sig_info
        .as_ref()
        .and_then(|info| info.return_type.clone())
        .or_else(|| extract_return_type_from_signature(sig).and_then(|s| KotlinType::parse(&s)))
        .or_else(|| {
            super::lambda_call_candidate::infer_decl_single_expression_return_type(
                index,
                url,
                decl,
                sig_info.as_ref(),
                depth + 1,
            )
        })
}

fn try_resolve_extension_candidate(
    ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) -> Option<KotlinType> {
    let sig = decl.signature.as_deref()?;
    if !is_valid_extension_receiver(
        ctx.index,
        ctx.current_url,
        container_type,
        url,
        decl,
        sig,
        ctx.tracer,
    ) {
        return None;
    }
    let origin = super::trace::format_origin(url);
    if let Some(ret) = extract_or_infer_extension_return_type(ctx.index, url, decl, sig, ctx.depth)
    {
        return Some(resolve_explicit_return_candidate(
            ctx,
            container_type,
            url,
            decl,
            &ret,
            call_info,
            inferred_arg_types,
        ));
    }

    if crate::indexer::is_source_url(url)
        && let Some(decl_source) = ctx.index.get_file_source(url)
        && let Some(inferred) = super::function::infer_single_expression_function_return_type(
            ctx.index,
            url,
            decl,
            &decl_source,
            ctx.depth + 1,
        )
    {
        ctx.tracer.record_step(|| {
            common::concat_str!(
                "Selected candidate `",
                decl.name.as_str(),
                "` (`",
                sig,
                "`) (inferred single-expression: `",
                inferred.to_string(),
                "`) ",
                origin
            )
        });
        return Some(inferred);
    }
    if decl.kind == SymbolKind::FUNCTION || decl.kind == SymbolKind::METHOD {
        ctx.tracer.record_step(|| {
            common::concat_str!(
                "Candidate `",
                decl.name.as_str(),
                "` (`",
                sig,
                "`) has no explicit return type -> fallback to Unit ",
                origin
            )
        });
        return Some(KotlinType::UNIT);
    }
    None
}
