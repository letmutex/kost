use tower_lsp::lsp_types::{SymbolKind, Url};

use super::ast_eval::AstEvalContext;
use super::signature::SignatureInfo;
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};

pub fn resolve_callable_reference(
    ctx: &mut AstEvalContext,
    lhs_resolved: Option<(KotlinType, bool)>,
    member_name: &str,
) -> Option<KotlinType> {
    let expected_inputs = match ctx.expected_type.as_ref() {
        Some(KotlinType::Function {
            receiver, params, ..
        }) => Some(
            receiver
                .iter()
                .map(|r| (**r).clone())
                .chain(params.iter().cloned())
                .collect::<Vec<_>>(),
        ),
        _ => None,
    };

    if let Some((receiver_ty, is_unbound_type)) = lhs_resolved {
        resolve_member_callable_reference(
            ctx,
            receiver_ty,
            is_unbound_type,
            member_name,
            expected_inputs.as_deref(),
        )
    } else {
        resolve_toplevel_callable_reference(ctx, member_name, expected_inputs.as_deref())
    }
}

fn is_companion_container_name(clean_container: &str, supertypes: &[String], cand: &str) -> bool {
    let short_base = clean_container
        .rsplit('.')
        .next()
        .unwrap_or(clean_container);
    cand != clean_container
        && cand != short_base
        && !supertypes.iter().any(|sup| {
            let sup_bare = sup.split('<').next().unwrap_or(sup).trim();
            let sup_short = sup_bare.rsplit('.').next().unwrap_or(sup_bare);
            cand == sup || cand == sup_bare || cand == sup_short
        })
}

fn is_container_decl_visible(
    index: &WorkspaceIndex,
    current_url: &Url,
    decl_url: &Url,
    decl: &DeclarationItem,
) -> bool {
    let is_ext = SignatureInfo::from_decl(decl).is_some_and(|info| info.receiver.is_some());
    if is_ext {
        super::qualification::is_declaration_visible(index, current_url, decl_url, decl)
    } else {
        !decl.is_private || decl_url == current_url
    }
}

fn find_callable_ref_in_containers(
    ctx: &AstEvalContext,
    receiver_ty: &KotlinType,
    is_unbound_type: bool,
    member_name: &str,
    expected_inputs: Option<&[KotlinType]>,
    supertypes: &[String],
) -> Option<KotlinType> {
    let clean_container = receiver_ty.base_name();
    let all_containers =
        super::member_ranking::collect_candidate_containers(clean_container, ctx.index);
    for container_name in &all_containers {
        let is_companion = is_companion_container_name(clean_container, supertypes, container_name);
        let effective_unbound = is_unbound_type && !is_companion;
        let expected_args = expected_inputs.and_then(|inputs| {
            if effective_unbound {
                inputs.get(1..)
            } else {
                Some(inputs)
            }
        });
        let Some(decls) = ctx.index.container_to_decls.get(container_name.as_str()) else {
            continue;
        };
        for (decl_url, decl) in decls.iter().filter(|(_, d)| d.name == member_name) {
            if is_container_decl_visible(ctx.index, ctx.current_url, decl_url.as_ref(), decl)
                && let Some(fn_ty) = build_callable_ref_function_type(
                    ctx.index,
                    decl_url.as_ref(),
                    decl,
                    Some(receiver_ty),
                    effective_unbound,
                    expected_args,
                )
            {
                return Some(fn_ty);
            }
        }
    }
    None
}

fn resolve_member_callable_reference(
    ctx: &mut AstEvalContext,
    mut receiver_ty: KotlinType,
    is_unbound_type: bool,
    member_name: &str,
    expected_inputs: Option<&[KotlinType]>,
) -> Option<KotlinType> {
    if is_unbound_type
        && receiver_ty.type_arguments().is_empty()
        && let Some(exp_rec) = expected_inputs.and_then(|inputs| inputs.first())
        && !exp_rec.type_arguments().is_empty()
        && exp_rec.base_name().rsplit('.').next() == receiver_ty.base_name().rsplit('.').next()
    {
        receiver_ty = exp_rec.clone();
    }
    let supertypes = ctx.index.get_all_supertypes(receiver_ty.base_name());
    if let Some(fn_ty) = find_callable_ref_in_containers(
        ctx,
        &receiver_ty,
        is_unbound_type,
        member_name,
        expected_inputs,
        &supertypes,
    ) {
        return Some(fn_ty);
    }
    let expected_member_args = expected_inputs.and_then(|inputs| {
        if is_unbound_type {
            inputs.get(1..)
        } else {
            Some(inputs)
        }
    });
    if let Some(decls) = ctx.index.name_to_decls.get(member_name) {
        for (decl_url, decl) in decls {
            if super::lambda_call_extension::is_extension_container_plausible(
                ctx.index,
                &receiver_ty,
                &supertypes,
                decl,
            ) && super::qualification::is_declaration_visible(
                ctx.index,
                ctx.current_url,
                decl_url.as_ref(),
                decl,
            ) && let Some(fn_ty) = build_callable_ref_function_type(
                ctx.index,
                decl_url.as_ref(),
                decl,
                Some(&receiver_ty),
                is_unbound_type,
                expected_member_args,
            ) {
                return Some(fn_ty);
            }
        }
    }
    if receiver_ty.unwrap_nullable_ref() == &KotlinType::STRING && member_name == "plus" {
        let rhs_ty = expected_member_args
            .and_then(|args| args.first().cloned())
            .unwrap_or_else(|| KotlinType::ANY.make_nullable());
        let params = if is_unbound_type {
            vec![receiver_ty, rhs_ty]
        } else {
            vec![rhs_ty]
        };
        return Some(KotlinType::function(
            None,
            params,
            KotlinType::STRING,
            false,
        ));
    }
    None
}

fn resolve_toplevel_callable_reference(
    ctx: &mut AstEvalContext,
    member_name: &str,
    expected_inputs: Option<&[KotlinType]>,
) -> Option<KotlinType> {
    let decls = ctx.index.name_to_decls.get(member_name)?;
    for (decl_url, decl) in decls {
        if decl.container_name.is_none()
            && super::qualification::is_declaration_visible(
                ctx.index,
                ctx.current_url,
                decl_url.as_ref(),
                decl,
            )
            && let Some(fn_ty) = build_callable_ref_function_type(
                ctx.index,
                decl_url.as_ref(),
                decl,
                None,
                false,
                expected_inputs,
            )
        {
            return Some(fn_ty);
        }
    }
    None
}

fn build_property_callable_ref_type(
    index: &WorkspaceIndex,
    decl_url: &Url,
    sig_info: &SignatureInfo,
    receiver_ty: Option<&KotlinType>,
    is_unbound_type: bool,
    expected_member_args: Option<&[KotlinType]>,
) -> Option<KotlinType> {
    if expected_member_args.is_some_and(|args| !args.is_empty()) {
        return None;
    }
    let mut system = ConstraintSystem::from_signature_info(index, sig_info);
    if let Some(rec) = receiver_ty {
        system.add_container_type_params_typed(rec);
        if let Some(ref decl_rec) = sig_info.receiver
            && !system.add_receiver_constraint_typed(rec, decl_rec)
        {
            return None;
        }
    } else if sig_info.receiver.is_some() {
        return None;
    }
    let ret_ty = sig_info.return_type.as_ref().map(|t| {
        super::qualification::qualify_type(index, decl_url, &system.solve_and_substitute(t))
    })?;
    let params = if is_unbound_type {
        receiver_ty.cloned().into_iter().collect()
    } else {
        Vec::new()
    };
    Some(KotlinType::function(None, params, ret_ty, false))
}

fn is_callable_ref_arity_compatible(
    sig_info: &SignatureInfo,
    expected_member_args: Option<&[KotlinType]>,
) -> bool {
    let Some(exp_args) = expected_member_args else {
        return true;
    };
    let required_count = sig_info
        .params
        .iter()
        .filter(|p| !p.has_default && !p.is_vararg)
        .count();
    let has_vararg = sig_info.params.iter().any(|p| p.is_vararg);
    exp_args.len() >= required_count && (has_vararg || exp_args.len() <= sig_info.params.len())
}

fn build_callable_ref_function_type(
    index: &WorkspaceIndex,
    decl_url: &Url,
    decl: &DeclarationItem,
    receiver_ty: Option<&KotlinType>,
    is_unbound_type: bool,
    expected_member_args: Option<&[KotlinType]>,
) -> Option<KotlinType> {
    let sig_info = SignatureInfo::from_decl(decl)?;
    if matches!(decl.kind, SymbolKind::PROPERTY | SymbolKind::FIELD) {
        return build_property_callable_ref_type(
            index,
            decl_url,
            &sig_info,
            receiver_ty,
            is_unbound_type,
            expected_member_args,
        );
    }
    if !matches!(
        decl.kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
    ) || (receiver_ty.is_none() && sig_info.receiver.is_some())
        || !is_callable_ref_arity_compatible(&sig_info, expected_member_args)
    {
        return None;
    }
    let mut system = ConstraintSystem::from_signature_info(index, &sig_info);
    if let Some(rec) = receiver_ty {
        system.add_container_type_params_typed(rec);
        if let Some(ref decl_rec) = sig_info.receiver
            && !system.add_receiver_constraint_typed(rec, decl_rec)
        {
            return None;
        }
    }
    if let Some(exp_args) = expected_member_args {
        for (idx, exp_arg) in exp_args.iter().enumerate() {
            if !matches!(exp_arg, KotlinType::TypeParam(_))
                && let Some(param) = sig_info.params.get(idx)
                && let Some(ref param_ty) = param.kotlin_type
                && !system.add_subtype_constraint(exp_arg, param_ty)
            {
                return None;
            }
        }
    }
    system.solve_in_place();
    let mut params = if is_unbound_type {
        vec![receiver_ty?.clone()]
    } else {
        Vec::new()
    };
    let param_count = expected_member_args
        .map(|a| a.len())
        .unwrap_or(sig_info.params.len());
    for param in sig_info.params.iter().take(param_count) {
        let raw_ty = param
            .kotlin_type
            .clone()
            .or_else(|| system.parse_with_params(&param.type_name))?;
        let solved = super::qualification::qualify_type(
            index,
            decl_url,
            &raw_ty.substitute(&system.bindings),
        );
        params.push(solved);
    }
    let raw_ret = sig_info.return_type.clone().unwrap_or(KotlinType::UNIT);
    let solved_ret =
        super::qualification::qualify_type(index, decl_url, &raw_ret.substitute(&system.bindings));
    Some(KotlinType::function(None, params, solved_ret, false))
}
