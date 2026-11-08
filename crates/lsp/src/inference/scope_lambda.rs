use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::expression::infer_receiver_type_depth;
pub use super::lambda_call_solve::get_builtin_scope_function;
use super::lambda_call_solve::resolve_fallback_type_param;
use super::overload::score_overload_candidate_parsed;
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex, is_source_url};
use crate::syntax::ast_call_view::{ParsedCallArg, parse_call_arg};

fn find_last_functional_param_in_sig(sig: &str, index: &WorkspaceIndex) -> Option<KotlinType> {
    let params = crate::inference::signature::parse_signature_params(sig);
    params.into_iter().rev().find_map(|p| {
        p.kotlin_type
            .filter(|kt| kt.is_function() || super::types::is_functional_or_sam(kt, index))
    })
}

pub(crate) struct LambdaSolveCtx<'a> {
    pub(crate) index: &'a WorkspaceIndex,
    pub(crate) current_url: &'a Url,
    pub(crate) receiver_type: Option<&'a KotlinType>,
    pub(crate) explicit_type_args: &'a [KotlinType],
    pub(crate) parsed_args: Vec<ParsedCallArg>,
    pub(crate) inferred_arg_types: Vec<Option<KotlinType>>,
    pub(crate) arg_idx: usize,
    pub(crate) is_trailing: bool,
    pub(crate) pos: Position,
    pub(crate) source: &'a str,
    pub(crate) root: Option<Node<'a>>,
    pub(crate) depth: usize,
}

impl<'a> LambdaSolveCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        index: &'a WorkspaceIndex,
        current_url: &'a Url,
        receiver_type: Option<&'a KotlinType>,
        explicit_type_args: &'a [KotlinType],
        value_args: &[&str],
        arg_idx: usize,
        is_trailing: bool,
        pos: Position,
        source: &'a str,
        root: Option<Node<'a>>,
        depth: usize,
    ) -> Self {
        let parsed_args: Vec<_> = value_args.iter().map(|a| parse_call_arg(a)).collect();
        let inferred_arg_types = parsed_args
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                if (!is_trailing && idx == arg_idx) || p.expr.trim_start().starts_with('{') {
                    None
                } else {
                    infer_receiver_type_depth(
                        index,
                        current_url,
                        &p.expr,
                        pos,
                        source,
                        root,
                        depth + 1,
                    )
                }
            })
            .collect();
        Self {
            index,
            current_url,
            receiver_type,
            explicit_type_args,
            parsed_args,
            inferred_arg_types,
            arg_idx,
            is_trailing,
            pos,
            source,
            root,
            depth,
        }
    }

    pub(crate) fn with_receiver(&self, receiver_type: &'a KotlinType) -> Self {
        Self {
            receiver_type: Some(receiver_type),
            parsed_args: self.parsed_args.clone(),
            inferred_arg_types: self.inferred_arg_types.clone(),
            ..*self
        }
    }

    fn solve_for_decl(&self, decl_url: &Url, decl: &DeclarationItem) -> Option<KotlinType> {
        let sig = decl.signature.as_deref()?;
        let sig_info = super::signature::SignatureInfo::from_decl(decl);
        self.solve_for_sig_and_info(decl_url, sig, sig_info.as_ref())
    }

    pub(crate) fn solve_for_signature(&self, decl_url: &Url, sig: &str) -> Option<KotlinType> {
        let sig_info = super::signature::SignatureInfo::parse(sig);
        self.solve_for_sig_and_info(decl_url, sig, sig_info.as_ref())
    }

    fn apply_receiver_and_arg_constraints(
        &self,
        system: &mut ConstraintSystem,
        sig: &str,
        sig_info: Option<&super::signature::SignatureInfo>,
    ) -> bool {
        if let Some(rec) = self.receiver_type {
            system.add_container_type_params_typed(rec);
            if let Some(decl_rec) = sig_info.and_then(|info| info.receiver.as_ref()) {
                if !system.add_receiver_constraint_typed(rec, decl_rec) {
                    return false;
                }
            } else if sig.contains('.')
                && crate::syntax::signature::extract_receiver_type_from_signature(sig).is_some()
                && !system.add_receiver_constraint_from_sig(sig, rec)
            {
                return false;
            }
        } else if sig_info.is_some_and(|info| info.receiver.is_some()) {
            return false;
        }
        if sig_info.is_some_and(|info| {
            super::overload::validate_call_arguments(&info.params, &self.parsed_args).is_none()
        }) {
            return false;
        }
        let mapped_params = sig_info
            .map(|info| super::overload::map_call_args_to_params(&info.params, &self.parsed_args));
        for (idx, arg_ty_opt) in self.inferred_arg_types.iter().enumerate() {
            let Some(arg_ty) = arg_ty_opt else { continue };
            let expected_opt = if let Some(ref mapped) = mapped_params {
                mapped.get(idx).and_then(|p| *p).and_then(|p| {
                    p.kotlin_type
                        .clone()
                        .or_else(|| system.parse_with_params(&p.type_name))
                })
            } else {
                crate::signature::extract_param_type_by_index(sig, idx)
                    .and_then(|p_str| system.parse_with_params(&p_str))
            };
            if let Some(expected) = expected_opt
                && !matches!(arg_ty.unwrap_nullable_ref(), KotlinType::Function { .. })
                && !system.add_subtype_constraint(arg_ty, &expected)
            {
                return false;
            }
        }
        true
    }

    fn extract_target_lambda_param_type(
        &self,
        system: &ConstraintSystem,
        sig: &str,
        sig_info: Option<&super::signature::SignatureInfo>,
    ) -> Option<KotlinType> {
        let get_param = |p: &super::signature::SignatureParam| {
            p.kotlin_type
                .clone()
                .or_else(|| system.parse_with_params(&p.type_name))
        };
        if let Some(info) = sig_info {
            if self.is_trailing {
                let idx = super::lambda_call_solve::map_trailing_lambda_param_index(
                    self.index,
                    &info.params,
                    self.arg_idx,
                );
                return super::signature::are_unpassed_parameters_compatible(
                    &info.params,
                    idx,
                    self.arg_idx,
                )
                .then(|| info.params.get(idx).and_then(get_param))
                .flatten();
            }
            let mapped = super::overload::map_call_args_to_params(&info.params, &self.parsed_args);
            return mapped
                .get(self.arg_idx)
                .copied()
                .flatten()
                .or_else(|| info.params.get(self.arg_idx))
                .and_then(get_param);
        }
        if self.is_trailing {
            find_last_functional_param_in_sig(sig, self.index)
        } else {
            crate::signature::extract_param_type_by_index(sig, self.arg_idx)
                .and_then(|p_str| system.parse_with_params(&p_str))
                .or_else(|| find_last_functional_param_in_sig(sig, self.index))
        }
    }

    fn solve_for_sig_and_info(
        &self,
        decl_url: &Url,
        sig: &str,
        sig_info: Option<&super::signature::SignatureInfo>,
    ) -> Option<KotlinType> {
        let mut system = if let Some(info) = sig_info {
            ConstraintSystem::from_signature_info(self.index, info)
        } else {
            ConstraintSystem::from_signature(self.index, sig)
        };
        if !super::lambda_call_solve::is_explicit_type_arg_count_valid(
            system.type_params.len(),
            self.explicit_type_args.len(),
        ) {
            return None;
        }
        for (param, arg_ty) in system
            .type_params
            .clone()
            .into_iter()
            .zip(self.explicit_type_args.iter())
        {
            system.bind(param, arg_ty.clone());
        }

        let target_ty = self.extract_target_lambda_param_type(&system, sig, sig_info)?;

        if (!target_ty.is_function() && !super::types::is_functional_or_sam(&target_ty, self.index))
            || !self.apply_receiver_and_arg_constraints(&mut system, sig, sig_info)
        {
            return None;
        }

        if let Some(info) = sig_info
            && !super::lambda_call_extension::check_type_param_bounds_satisfaction(
                self.index,
                &info.type_params,
                &system.bindings,
            )
        {
            return None;
        }
        let mut solved = system.solve_and_substitute(&target_ty);
        if let KotlinType::Function {
            receiver,
            params,
            return_type,
            is_suspend,
        } = solved
        {
            let fallback_params = params
                .into_iter()
                .map(|param_ty| resolve_fallback_type_param(param_ty, self.receiver_type))
                .collect();
            solved = KotlinType::Function {
                receiver,
                params: fallback_params,
                return_type,
                is_suspend,
            };
        }
        Some(super::qualification::qualify_type(
            self.index, decl_url, &solved,
        ))
    }

    pub(crate) fn sort_and_solve_candidates(
        &self,
        mut candidates: Vec<(&Url, &DeclarationItem)>,
    ) -> Option<KotlinType> {
        if !self.parsed_args.is_empty() && candidates.len() > 1 {
            candidates.sort_by_cached_key(|(url, decl)| {
                std::cmp::Reverse(score_overload_candidate_parsed(
                    decl,
                    is_source_url(url),
                    &self.parsed_args,
                    &self.inferred_arg_types,
                    self.index,
                ))
            });
        }
        for (decl_url, decl) in candidates {
            if let Some(k_ty) = self.solve_for_decl(decl_url, decl) {
                return Some(k_ty);
            }
        }
        None
    }
}

pub use super::scope_enclosing_lambda::{
    resolve_enclosing_lambda_type, resolve_lambda_signature_for_receiver,
    resolve_lambda_type_for_call_and_lambda,
};
