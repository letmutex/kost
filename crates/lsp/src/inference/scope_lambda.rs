use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::infer_receiver_type_depth;
pub use super::lambda_call_solve::get_builtin_scope_function;
use super::lambda_call_solve::resolve_fallback_type_param;
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

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
    pub(crate) value_args: &'a [&'a str],
    pub(crate) arg_idx: usize,
    pub(crate) is_trailing: bool,
    pub(crate) pos: Position,
    pub(crate) source: &'a str,
    pub(crate) root: Option<Node<'a>>,
    pub(crate) depth: usize,
}

impl<'a> LambdaSolveCtx<'a> {
    fn solve_for_decl(
        &self,
        decl_url: &Url,
        decl: &crate::indexer::DeclarationItem,
    ) -> Option<KotlinType> {
        let sig = decl.signature.as_deref()?;
        let sig_info = super::signature::SignatureInfo::from_decl(decl);
        self.solve_for_sig_and_info(decl_url, sig, sig_info.as_ref())
    }

    fn solve_for_signature(&self, decl_url: &Url, sig: &str) -> Option<KotlinType> {
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

        for (arg_pos_idx, arg_expr) in self.value_args.iter().enumerate() {
            let parsed_arg = crate::syntax::ast_call_view::parse_call_arg(arg_expr);
            let Some(arg_ty) = infer_receiver_type_depth(
                self.index,
                self.current_url,
                &parsed_arg.expr,
                self.pos,
                self.source,
                self.root,
                self.depth + 1,
            ) else {
                continue;
            };
            let expected_opt = if let Some(info) = sig_info {
                info.params
                    .get(arg_pos_idx)
                    .and_then(|p| p.kotlin_type.clone())
            } else {
                crate::signature::extract_param_type_by_index(sig, arg_pos_idx)
                    .and_then(|p_str| system.parse_with_params(&p_str))
            };
            if let Some(expected) = expected_opt {
                system.add_subtype_constraint(&arg_ty, &expected);
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
        if self.is_trailing {
            sig_info
                .and_then(|info| {
                    let idx = super::lambda_call_solve::map_trailing_lambda_param_index(
                        self.index,
                        &info.params,
                        self.arg_idx,
                    );
                    info.params.get(idx).and_then(|p| {
                        p.kotlin_type
                            .clone()
                            .or_else(|| system.parse_with_params(&p.type_name))
                    })
                })
                .or_else(|| find_last_functional_param_in_sig(sig, self.index))
        } else {
            sig_info
                .and_then(|info| {
                    info.params.get(self.arg_idx).and_then(|p| {
                        p.kotlin_type
                            .clone()
                            .or_else(|| system.parse_with_params(&p.type_name))
                    })
                })
                .or_else(|| {
                    crate::signature::extract_param_type_by_index(sig, self.arg_idx)
                        .and_then(|p_str| system.parse_with_params(&p_str))
                })
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

    fn find_in_containers(&self, containers: &[String], method_name: &str) -> Option<KotlinType> {
        for container_name in containers {
            if let Some(decls) = self.index.container_to_decls.get(container_name.as_str()) {
                for (decl_url, decl) in decls.iter().filter(|(_, d)| d.name == method_name) {
                    let is_ext = super::signature::SignatureInfo::from_decl(decl)
                        .is_some_and(|info| info.receiver.is_some());
                    let is_visible = if is_ext {
                        super::qualification::is_declaration_visible(
                            self.index,
                            self.current_url,
                            decl_url.as_ref(),
                            decl,
                        )
                    } else {
                        !decl.is_private || decl_url.as_ref() == self.current_url
                    };
                    if is_visible && let Some(k_ty) = self.solve_for_decl(decl_url.as_ref(), decl) {
                        return Some(k_ty);
                    }
                }
            }
        }
        None
    }

    fn find_in_declarations_or_builtin(
        &self,
        name: &str,
        require_function: bool,
        supertypes: &[String],
    ) -> Option<KotlinType> {
        if let Some(decls) = self.index.name_to_decls.get(name) {
            for (decl_url, decl) in decls {
                let is_ok = matches!(
                    decl.kind,
                    SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
                ) && (!require_function || decl.kind == SymbolKind::FUNCTION)
                    && self.receiver_type.is_none_or(|rec| {
                        super::lambda_call_extension::is_extension_container_plausible(
                            self.index, rec, supertypes, decl,
                        )
                    })
                    && super::qualification::is_declaration_visible(
                        self.index,
                        self.current_url,
                        decl_url.as_ref(),
                        decl,
                    );
                if is_ok && let Some(k_ty) = self.solve_for_decl(decl_url.as_ref(), decl) {
                    return Some(k_ty);
                }
            }
        }
        let builtin_sig = get_builtin_scope_function(name)?;
        self.solve_for_signature(self.current_url, builtin_sig)
    }
}

pub(crate) fn resolve_member_lambda_type(
    ctx: &LambdaSolveCtx,
    receiver_type: &KotlinType,
    method_name: &str,
) -> Option<KotlinType> {
    let clean_container = receiver_type.base_name();
    let supertypes = ctx.index.get_all_supertypes(clean_container);
    let all_containers =
        super::member_ranking::collect_candidate_containers(clean_container, ctx.index);

    ctx.find_in_containers(&all_containers, method_name)
        .or_else(|| ctx.find_in_declarations_or_builtin(method_name, false, &supertypes))
}

pub(crate) fn resolve_toplevel_lambda_type(
    ctx: &LambdaSolveCtx,
    callee_name: &str,
) -> Option<KotlinType> {
    if let Some(res) = ctx.find_in_declarations_or_builtin(callee_name, true, &[]) {
        return Some(res);
    }
    for implicit_rec in super::scope_stack::collect_implicit_receivers(
        ctx.index,
        ctx.current_url,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    ) {
        let member_ctx = LambdaSolveCtx {
            receiver_type: Some(&implicit_rec),
            ..*ctx
        };
        if let Some(res) = resolve_member_lambda_type(&member_ctx, &implicit_rec, callee_name) {
            return Some(res);
        }
    }
    None
}

pub fn resolve_lambda_signature_for_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    receiver_type: Option<&KotlinType>,
    callee: &str,
    explicit_type_args: &[KotlinType],
    value_args: &[&str],
    arg_idx: usize,
    is_trailing: bool,
) -> Option<KotlinType> {
    let solve_ctx = LambdaSolveCtx {
        index: ctx.index,
        current_url: ctx.current_url,
        receiver_type,
        explicit_type_args,
        value_args,
        arg_idx,
        is_trailing,
        pos: ctx.pos,
        source: ctx.source,
        root: ctx.root,
        depth: ctx.depth,
    };
    if let Some(rec) = receiver_type {
        resolve_member_lambda_type(&solve_ctx, rec, callee)
    } else {
        resolve_toplevel_lambda_type(&solve_ctx, callee)
    }
}

pub use super::scope_enclosing_lambda::{
    resolve_enclosing_lambda_type, resolve_lambda_type_for_call_and_lambda,
};
