use tower_lsp::lsp_types::Url;

use super::hierarchy::is_in_container_hierarchy;
use crate::indexer::WorkspaceIndex;
use crate::inference::types::KotlinType;

pub(super) fn is_type_conforming(
    index: &WorkspaceIndex,
    cand_url: &Url,
    arg_type: &KotlinType,
    param_ty: &str,
    type_params: &[&str],
) -> bool {
    let Some(parsed_param) =
        crate::inference::types::parse_type_signature_with_params(param_ty, type_params)
    else {
        let full_arg =
            crate::inference::resolve_full_type_name(index, cand_url, arg_type.base_name());
        return is_in_container_hierarchy(index, &full_arg, param_ty.trim());
    };

    is_kotlin_type_conforming(index, cand_url, arg_type, &parsed_param)
}

pub(super) fn is_kotlin_type_conforming(
    index: &WorkspaceIndex,
    cand_url: &Url,
    arg_type: &KotlinType,
    param_type: &KotlinType,
) -> bool {
    if arg_type.is_nullable() && !param_type.is_nullable() {
        return false;
    }

    match (
        arg_type.unwrap_nullable_ref(),
        param_type.unwrap_nullable_ref(),
    ) {
        (_, KotlinType::TypeParam(_)) => true,
        (
            KotlinType::Function {
                params: actual_params,
                return_type: actual_ret,
                ..
            },
            KotlinType::Function {
                params: param_params,
                return_type: param_ret,
                ..
            },
        ) => {
            if actual_params.is_empty() {
                if param_params.len() > 1 {
                    return false;
                }
            } else if actual_params.len() != param_params.len() {
                return false;
            }
            if !actual_params.is_empty()
                && !actual_params.iter().zip(param_params.iter()).all(|(a, p)| {
                    a.is_nullable() == p.is_nullable()
                        && (is_kotlin_type_conforming(index, cand_url, p, a)
                            || is_kotlin_type_conforming(index, cand_url, a, p))
                })
            {
                return false;
            }
            if param_ret.is_unit() {
                return true;
            }
            is_kotlin_type_conforming(index, cand_url, actual_ret, param_ret)
        }
        _ => {
            let clean_param = param_type.unwrap_nullable_ref().base_name();
            let full_arg =
                crate::inference::resolve_full_type_name(index, cand_url, arg_type.base_name());
            is_in_container_hierarchy(index, &full_arg, clean_param)
        }
    }
}

pub(super) fn is_functional_type_name(type_name: &str) -> bool {
    KotlinType::parse(type_name).is_some_and(|kt| kt.is_function())
}
