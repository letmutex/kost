use std::cmp::Reverse;

use super::bounds::is_subtype;
use super::candidate::{
    TypedCandidate, generic_permits_nullable_typed, is_unconstrained_generic_typed,
};
use super::{OverloadScore, collect_container_type_params, validate_call_arguments};
use crate::indexer::WorkspaceIndex;
use crate::inference::signature::{GenericParamInfo, SignatureParam};
use crate::inference::types::standard::StandardInterface;
use crate::inference::types::{KotlinType, Variance};
use crate::syntax::ast_call_view::ParsedCallArg;

fn score_class_args(
    arg_ty: &KotlinType,
    tgt_ty: &KotlinType,
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> Option<usize> {
    let a_args = arg_ty.type_arguments();
    let t_args = tgt_ty.type_arguments();
    if a_args.is_empty() || a_args.len() != t_args.len() {
        return None;
    }
    let mut sum = 0;
    for (a, t) in a_args.iter().zip(t_args.iter()) {
        if t.variance == Variance::Star {
            sum += 1;
            continue;
        }
        let sub = type_match_score(&a.type_ref, &t.type_ref, index, type_params)?;
        sum += sub;
    }
    Some((sum / a_args.len()).max(1))
}

/// Compute typed match score between an argument type and a parameter type
pub fn type_match_score(
    arg_ty: &KotlinType,
    target_ty: &KotlinType,
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> Option<usize> {
    let arg_nullable = arg_ty.is_nullable();
    let tgt_nullable = target_ty.is_nullable();

    let simple_tgt = target_ty.base_name();
    let simple_tgt_short = simple_tgt.rsplit('.').next().unwrap_or(simple_tgt);

    if arg_nullable
        && !tgt_nullable
        && !generic_permits_nullable_typed(simple_tgt_short, type_params, index)
    {
        return None;
    }

    if arg_ty == target_ty || arg_ty.unwrap_nullable_ref() == target_ty.unwrap_nullable_ref() {
        if !arg_nullable && tgt_nullable {
            return Some(3);
        }
        return Some(4);
    }

    if target_ty.is_any() {
        return Some(1);
    }

    if let Some(res) =
        score_generic_param(simple_tgt_short, arg_ty, tgt_nullable, index, type_params)
    {
        return res;
    }

    let arg_base = arg_ty.base_name();
    let tgt_base = target_ty.base_name();
    let arg_s = arg_base.rsplit('.').next().unwrap_or(arg_base);
    let tgt_s = tgt_base.rsplit('.').next().unwrap_or(tgt_base);

    if (arg_s == tgt_s || arg_base == tgt_base)
        && (!arg_ty.type_arguments().is_empty() || !target_ty.type_arguments().is_empty())
    {
        return score_class_args(arg_ty, target_ty, index, type_params);
    }

    let supertypes = index.get_all_supertypes(arg_base);
    if supertypes.iter().any(|s| {
        let sb = s.split('<').next().unwrap_or(s).trim();
        let ss = sb.rsplit('.').next().unwrap_or(sb);
        sb == tgt_base || ss == tgt_s
    }) {
        return Some(3);
    }

    if is_subtype(arg_ty, target_ty, index, type_params) {
        return Some(3);
    }

    None
}

/// Check if actual type is structurally incompatible with expected primitive type
pub fn is_primitive_incompatible(
    actual: &KotlinType,
    expected: &KotlinType,
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> bool {
    const P: &[&str] = &[
        "Int", "Long", "Double", "Float", "Boolean", "String", "Char", "Byte", "Short",
    ];

    let actual_nullable = actual.is_nullable();
    let expected_nullable = expected.is_nullable();
    let simple_exp = expected.base_name();
    let simple_exp_short = simple_exp.rsplit('.').next().unwrap_or(simple_exp);
    let permits_nullable = generic_permits_nullable_typed(simple_exp_short, type_params, index);

    if actual_nullable && !expected_nullable && !permits_nullable {
        return true;
    }

    if matches!(actual.unwrap_nullable_ref(), KotlinType::Nothing) {
        return false;
    }

    if let Some(param_info) = type_params.iter().find(|p| p.name == simple_exp_short) {
        if param_info.bounds.is_empty() {
            return false;
        }
        let check_ty = if expected_nullable {
            actual.unwrap_nullable_ref()
        } else {
            actual
        };
        return !param_info
            .bounds
            .iter()
            .all(|b| is_subtype(check_ty, b, index, type_params));
    }

    if is_unconstrained_generic_typed(simple_exp_short, type_params, index) {
        return false;
    }

    let simple_act = actual.base_name();
    let simple_act_short = simple_act.rsplit('.').next().unwrap_or(simple_act);

    let actual_is_p = P.contains(&simple_act_short);
    let expected_is_p = P.contains(&simple_exp_short);
    if actual_is_p && expected_is_p && simple_act_short != simple_exp_short {
        return true;
    }

    if (!actual.type_arguments().is_empty()) && (!expected.type_arguments().is_empty()) {
        let a_args = actual.type_arguments();
        let e_args = expected.type_arguments();
        if a_args.len() == e_args.len() {
            for (a, e) in a_args.iter().zip(e_args.iter()) {
                if is_primitive_incompatible(&a.type_ref, &e.type_ref, index, type_params) {
                    return true;
                }
            }
        }
    }

    let is_std_iface = StandardInterface::from_name(simple_exp_short).is_some();
    let actual_is_any =
        StandardInterface::from_name(simple_act_short) == Some(StandardInterface::Any);
    if (actual_is_p && !expected_is_p && !is_std_iface)
        || (expected_is_p && !actual_is_p && !actual_is_any)
    {
        return true;
    }
    false
}

/// Compute type matches over pre-mapped parameters using typed KotlinType scoring
pub fn compute_type_matches(
    params: &[SignatureParam],
    call_args: &[ParsedCallArg],
    inferred_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> Option<usize> {
    let mapping = super::map_call_args_to_params(params, call_args);
    let mut type_matches = 0;

    for (idx, mapped) in mapping.into_iter().enumerate() {
        if let Some(param) = mapped
            && let Some(Some(arg_inferred)) = inferred_types.get(idx)
        {
            let target_ty = param
                .kotlin_type
                .clone()
                .or_else(|| KotlinType::parse(param.type_name.as_str()))
                .unwrap_or(KotlinType::DynamicOrAny);
            if let Some(score) = type_match_score(arg_inferred, &target_ty, index, type_params) {
                type_matches += score;
            } else if is_primitive_incompatible(arg_inferred, &target_ty, index, type_params) {
                return None;
            }
        }
    }

    Some(type_matches)
}

/// Score an ephemeral typed candidate against pre-parsed call arguments
pub fn score_typed_candidate(
    candidate: &TypedCandidate<'_>,
    is_src: bool,
    call_args: &[ParsedCallArg],
    inferred_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) -> Option<OverloadScore> {
    let params = &candidate.sig_info.params;
    let mut type_params = candidate.sig_info.type_params.clone();
    let named_matches = validate_call_arguments(params, call_args)?;

    if let Some(ref container) = candidate.decl.container_name {
        collect_container_type_params(container, index, &mut type_params);
    }

    let type_matches =
        compute_type_matches(params, call_args, inferred_types, index, &type_params)?;

    let exact_arity = params.len() == call_args.len();
    let param_count_diff = params.len().abs_diff(call_args.len());

    Some(OverloadScore {
        type_matches,
        named_matches,
        exact_arity,
        param_count_diff_penalty: Reverse(param_count_diff),
        is_source: is_src,
    })
}

fn score_generic_param(
    simple_tgt_short: &str,
    arg_ty: &KotlinType,
    tgt_nullable: bool,
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> Option<Option<usize>> {
    if let Some(param_info) = type_params.iter().find(|p| p.name == simple_tgt_short) {
        if param_info.bounds.is_empty() {
            return Some(Some(2));
        }
        let check_ty = if tgt_nullable {
            arg_ty.unwrap_nullable_ref()
        } else {
            arg_ty
        };
        if param_info
            .bounds
            .iter()
            .all(|b| is_subtype(check_ty, b, index, type_params))
        {
            return Some(Some(3));
        }
        return Some(None);
    }

    if is_unconstrained_generic_typed(simple_tgt_short, type_params, index) {
        return Some(Some(2));
    }
    None
}
