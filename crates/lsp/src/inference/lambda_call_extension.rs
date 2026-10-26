use ahash::AHashMap;
use smol_str::SmolStr;
use tower_lsp::lsp_types::{SymbolKind, Url};

use super::overload::TypedCandidate;
use super::qualification::is_declaration_visible;
use super::signature::{GenericParamInfo, SignatureInfo};
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex, is_source_url};

pub struct ExtensionCandidate<'a> {
    pub decl: &'a DeclarationItem,
    pub url: &'a Url,
    pub sig_info: SignatureInfo,
    pub is_source: bool,
}

pub fn is_callable_function_decl(decl: &DeclarationItem) -> bool {
    matches!(
        decl.kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
    )
}

fn build_skolem_env(
    tps_sub: &[GenericParamInfo],
    prefix_sub: &str,
) -> (
    AHashMap<SmolStr, KotlinType>,
    AHashMap<SmolStr, Vec<KotlinType>>,
) {
    let mut skolem_bindings = AHashMap::default();
    let mut skolem_bounds = AHashMap::default();
    for param in tps_sub {
        let skolem_name = common::concat_smol_str!("$Skolem_", prefix_sub, "_", param.name);
        skolem_bindings.insert(param.name.clone(), KotlinType::simple_class(skolem_name));
    }
    for param in tps_sub {
        let skolem_name = common::concat_smol_str!("$Skolem_", prefix_sub, "_", param.name);
        if !param.bounds.is_empty() {
            let subst_bounds: Vec<_> = param
                .bounds
                .iter()
                .map(|b| b.substitute(&skolem_bindings))
                .collect();
            skolem_bounds.insert(skolem_name, subst_bounds);
        }
    }
    (skolem_bindings, skolem_bounds)
}

pub fn check_type_param_bounds_satisfaction(
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
    bindings: &AHashMap<SmolStr, KotlinType>,
) -> bool {
    for param in type_params {
        if let Some(bound_ty) = bindings.get(&param.name) {
            for bound in &param.bounds {
                let subst_bound = bound.substitute(bindings);
                if subst_bound.is_any() && subst_bound.is_nullable() {
                    continue;
                }
                if bound_ty.is_nullable() && !subst_bound.is_nullable() {
                    return false;
                }
                if !super::overload::is_subtype(bound_ty, &subst_bound, index, type_params) {
                    return false;
                }
            }
        }
    }
    true
}

fn check_skolem_bound(
    index: &WorkspaceIndex,
    bound_ty: &KotlinType,
    skolem_bounds: &AHashMap<SmolStr, Vec<KotlinType>>,
    subst_sup_bound: &KotlinType,
) -> bool {
    let skolem_name = bound_ty.unwrap_nullable_ref().base_name();
    let Some(declared_sub_bounds) = skolem_bounds.get(skolem_name) else {
        return false;
    };
    declared_sub_bounds.iter().any(|declared_bound| {
        let effective_bound = if bound_ty.is_nullable() {
            declared_bound.clone().make_nullable()
        } else {
            declared_bound.clone()
        };
        super::overload::is_subtype(&effective_bound, subst_sup_bound, index, &[])
    })
}

fn check_bound_satisfaction(
    index: &WorkspaceIndex,
    bound_ty: &KotlinType,
    sup_bounds: &[KotlinType],
    skolem_bounds: &AHashMap<SmolStr, Vec<KotlinType>>,
    bindings: &AHashMap<SmolStr, KotlinType>,
) -> bool {
    let clean_bound_ty = bound_ty.unwrap_nullable_ref();
    let is_skolem = if let KotlinType::Class { name, .. } = clean_bound_ty {
        name.starts_with("$Skolem_")
    } else {
        false
    };

    for sup_bound in sup_bounds {
        let subst_sup_bound = sup_bound.substitute(bindings);
        if subst_sup_bound.is_any() && subst_sup_bound.is_nullable() {
            continue;
        }
        if bound_ty.is_nullable() && !subst_sup_bound.is_nullable() {
            return false;
        }

        if is_skolem {
            if !check_skolem_bound(index, bound_ty, skolem_bounds, &subst_sup_bound) {
                return false;
            }
        } else if !super::overload::is_subtype(bound_ty, &subst_sup_bound, index, &[]) {
            return false;
        }
    }
    true
}

fn check_skolem_subtyping(
    index: &WorkspaceIndex,
    rec_sub: &KotlinType,
    tps_sub: &[GenericParamInfo],
    prefix_sub: &str,
    rec_sup: &KotlinType,
    tps_sup: &[GenericParamInfo],
) -> bool {
    let (skolem_bindings, skolem_bounds) = build_skolem_env(tps_sub, prefix_sub);
    let skolem_sub = rec_sub.substitute(&skolem_bindings);

    let sup_param_names: Vec<SmolStr> = tps_sup.iter().map(|param| param.name.clone()).collect();
    let mut system = ConstraintSystem::new(index, sup_param_names);

    if !system.add_subtype_constraint(&skolem_sub, rec_sup) {
        return false;
    }

    let bindings = system.solve();
    for param in tps_sup {
        if let Some(bound_ty) = bindings.get(&param.name)
            && !check_bound_satisfaction(index, bound_ty, &param.bounds, &skolem_bounds, &bindings)
        {
            return false;
        }
    }
    true
}

pub fn is_receiver_more_specific(
    index: &WorkspaceIndex,
    sig_info_a: &SignatureInfo,
    sig_info_b: &SignatureInfo,
) -> bool {
    let (Some(rec_a), Some(rec_b)) = (sig_info_a.receiver.as_ref(), sig_info_b.receiver.as_ref())
    else {
        return false;
    };
    let a_sub_b = check_skolem_subtyping(
        index,
        rec_a,
        &sig_info_a.type_params,
        "A",
        rec_b,
        &sig_info_b.type_params,
    );
    let b_sub_a = check_skolem_subtyping(
        index,
        rec_b,
        &sig_info_b.type_params,
        "B",
        rec_a,
        &sig_info_a.type_params,
    );
    a_sub_b && !b_sub_a
}

pub fn is_receiver_compatible_with_bounds(
    index: &WorkspaceIndex,
    actual_receiver: &KotlinType,
    decl_receiver: &KotlinType,
    sig_info: &SignatureInfo,
) -> bool {
    let mut system = ConstraintSystem::from_signature_info(index, sig_info);
    if !system.add_receiver_constraint_typed(actual_receiver, decl_receiver) {
        return false;
    }
    check_type_param_bounds_satisfaction(index, &sig_info.type_params, &system.bindings)
}

fn rank_candidates<'a>(
    index: &WorkspaceIndex,
    candidates: Vec<ExtensionCandidate<'a>>,
) -> Vec<ExtensionCandidate<'a>> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let mut dominance_counts = vec![0usize; candidates.len()];
    for i in 0..candidates.len() {
        for j in (i + 1)..candidates.len() {
            if is_receiver_more_specific(index, &candidates[j].sig_info, &candidates[i].sig_info) {
                dominance_counts[i] += 1;
            } else if is_receiver_more_specific(
                index,
                &candidates[i].sig_info,
                &candidates[j].sig_info,
            ) {
                dominance_counts[j] += 1;
            }
        }
    }

    let mut ranked: Vec<(usize, ExtensionCandidate<'a>)> = candidates
        .into_iter()
        .enumerate()
        .map(|(idx, candidate)| (dominance_counts[idx], candidate))
        .collect();
    ranked.sort_by(|(d1, c1), (d2, c2)| {
        d1.cmp(d2)
            .then_with(|| c1.is_source.cmp(&c2.is_source).reverse())
    });
    ranked.into_iter().map(|(_, candidate)| candidate).collect()
}

pub fn collect_ranked_extension_candidates<'a>(
    index: &'a WorkspaceIndex,
    current_url: &Url,
    receiver_type: &KotlinType,
    callee: &str,
) -> Vec<ExtensionCandidate<'a>> {
    let Some(decls) = index.name_to_decls.get(callee) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for (url, decl) in decls {
        if !is_callable_function_decl(decl)
            || !is_declaration_visible(index, current_url, url.as_ref(), decl)
        {
            continue;
        }
        let candidate = TypedCandidate::new(decl);
        let Some(ref decl_rec) = candidate.sig_info.receiver else {
            continue;
        };
        if !is_receiver_compatible_with_bounds(index, receiver_type, decl_rec, &candidate.sig_info)
        {
            continue;
        }
        candidates.push(ExtensionCandidate {
            decl: decl.as_ref(),
            url: url.as_ref(),
            sig_info: candidate.sig_info,
            is_source: is_source_url(url),
        });
    }

    rank_candidates(index, candidates)
}
