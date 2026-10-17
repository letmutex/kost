use tower_lsp::lsp_types::Url;

use super::lambda_call_extension::check_type_param_bounds_satisfaction;
use super::qualification::qualify_type;
use super::signature::{SignatureInfo, SignatureParam, are_unpassed_parameters_compatible};
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

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
