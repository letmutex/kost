use ahash::AHashMap;
use smol_str::SmolStr;

use super::solver::{ConstraintSystem, TypeConstraint};
use super::types::{KotlinType, TypeArg, Variance};
use super::unify::{is_class_subtype, unify_two_types};
use crate::indexer::WorkspaceIndex;

impl<'a> ConstraintSystem<'a> {
    pub(crate) fn is_decl_site_out(&self, class_name: &str, idx: usize) -> bool {
        let Some(decls) = self.index.name_to_decls.get(class_name) else {
            return false;
        };
        for (_, d) in decls {
            if let Some(sig_info) = super::signature::SignatureInfo::from_decl(d)
                && let Some(tp) = sig_info.type_params.get(idx)
            {
                return tp.is_out;
            }
        }
        false
    }

    pub(crate) fn instantiate_super_args(
        &self,
        sub_name: &str,
        sub_args: &[TypeArg],
        super_ty: &KotlinType,
    ) -> KotlinType {
        if sub_args.is_empty() {
            return super_ty.clone();
        }
        let Some(decls) = self.index.name_to_decls.get(sub_name) else {
            return super_ty.clone();
        };
        for (_, d) in decls {
            if let Some(sig_info) = super::signature::SignatureInfo::from_decl(d)
                && !sig_info.type_params.is_empty()
            {
                let mut bindings = AHashMap::default();
                for (tp, arg) in sig_info.type_params.iter().zip(sub_args.iter()) {
                    bindings.insert(tp.name.clone(), arg.type_ref.clone());
                }
                return super_ty.substitute(&bindings);
            }
        }
        super_ty.clone()
    }

    fn unify_via_generic_supertypes(
        &mut self,
        n1_s: &str,
        a1: &[TypeArg],
        n2: &str,
        n2_s: &str,
        a2: &[TypeArg],
    ) -> bool {
        if self
            .visited_classes
            .iter()
            .any(|(a, b)| a.as_str() == n1_s && b.as_str() == n2_s)
            || self.visited_classes.len() > crate::inference::MAX_INFERENCE_DEPTH
        {
            return false;
        }
        self.visited_classes
            .push((SmolStr::new(n1_s), SmolStr::new(n2_s)));
        let mut matched = false;
        for super_ty in super::supertypes::get_generic_supertypes(self.index, n1_s) {
            let inst = self.instantiate_super_args(n1_s, a1, &super_ty);
            if let KotlinType::Class {
                name: ref sn,
                args: ref sa,
            } = inst
                && self.unify_class_types(sn, sa, n2, a2)
            {
                matched = true;
                break;
            }
        }
        self.visited_classes.pop();
        matched
    }

    pub(crate) fn unify_class_types(
        &mut self,
        n1: &str,
        a1: &[TypeArg],
        n2: &str,
        a2: &[TypeArg],
    ) -> bool {
        let (n1_clean, n2_clean) = (
            n1.split('<').next().unwrap_or(n1).trim(),
            n2.split('<').next().unwrap_or(n2).trim(),
        );
        let (n1_s, n2_s) = (
            n1_clean.rsplit('.').next().unwrap_or(n1_clean),
            n2_clean.rsplit('.').next().unwrap_or(n2_clean),
        );
        if n1_clean == n2_clean || n1_s == n2_s {
            if a1.len() != a2.len() {
                return a2.is_empty();
            }
            let is_cov = super::types::standard::StandardInterface::from_name(n2_s)
                .is_some_and(|iface| iface.is_collection());
            return a1
                .iter()
                .zip(a2.iter())
                .enumerate()
                .all(|(idx, (arg1, arg2))| {
                    let v = if is_cov
                        || arg2.variance == Variance::Out
                        || self.is_decl_site_out(n2_s, idx)
                    {
                        Variance::Out
                    } else {
                        arg2.variance
                    };
                    match v {
                        Variance::Invariant => {
                            self.add_subtype_constraint(&arg1.type_ref, &arg2.type_ref)
                                && self.add_subtype_constraint(&arg2.type_ref, &arg1.type_ref)
                        }
                        Variance::Out => {
                            self.add_subtype_constraint(&arg1.type_ref, &arg2.type_ref)
                        }
                        Variance::In => self.add_subtype_constraint(&arg2.type_ref, &arg1.type_ref),
                        Variance::Star => true,
                    }
                });
        }
        if self.unify_via_generic_supertypes(n1_s, a1, n2, n2_s, a2) {
            return true;
        }
        is_class_subtype(self.index, n1_clean, n2_clean) && a2.is_empty()
    }
}

fn reconcile_existing_bound(
    index: &WorkspaceIndex,
    bound_ty: &KotlinType,
    substituted_bound: &KotlinType,
) -> Option<KotlinType> {
    let mut checker = ConstraintSystem::new(index, Vec::new());
    if checker.add_subtype_constraint(bound_ty, substituted_bound) {
        return None;
    }
    let fallback = unify_two_types(index, bound_ty, substituted_bound)
        .filter(|u| *u != KotlinType::ANY || substituted_bound == &KotlinType::ANY)
        .unwrap_or_else(|| substituted_bound.clone());
    Some(fallback)
}

fn contains_type_param(ty: &KotlinType, target_param: &str) -> bool {
    match ty {
        KotlinType::TypeParam(name) => name.as_str() == target_param,
        KotlinType::Class { name, args } => {
            (args.is_empty() && name.as_str() == target_param)
                || args
                    .iter()
                    .any(|arg| contains_type_param(&arg.type_ref, target_param))
        }
        KotlinType::Function {
            receiver,
            params,
            return_type,
            ..
        } => {
            receiver
                .as_ref()
                .is_some_and(|rec| contains_type_param(rec, target_param))
                || params
                    .iter()
                    .any(|param| contains_type_param(param, target_param))
                || contains_type_param(return_type, target_param)
        }
        KotlinType::Nullable(inner) => contains_type_param(inner, target_param),
        KotlinType::Intersection(types) => types
            .iter()
            .any(|inner| contains_type_param(inner, target_param)),
        KotlinType::Primitive(_) | KotlinType::Nothing | KotlinType::DynamicOrAny => false,
    }
}

impl<'a> ConstraintSystem<'a> {
    fn apply_single_upper_constraint(&mut self, constraint: &TypeConstraint) -> bool {
        let substituted = constraint.bound.substitute(&self.bindings);
        if contains_type_param(&substituted, &constraint.type_param) {
            return false;
        }
        if let Some(bound_ty) = self.bindings.get(&constraint.type_param).cloned() {
            if let KotlinType::TypeParam(ref target_param) = substituted {
                if self.type_params.iter().any(|p| p == target_param)
                    && !self.bindings.contains_key(target_param)
                {
                    self.bindings.insert(target_param.clone(), bound_ty);
                    return true;
                }
            } else if let Some(fallback) =
                reconcile_existing_bound(self.index, &bound_ty, &substituted)
            {
                self.bindings
                    .insert(constraint.type_param.clone(), fallback);
                return true;
            }
        } else if !matches!(substituted, KotlinType::TypeParam(_)) {
            self.bindings
                .insert(constraint.type_param.clone(), substituted);
            return true;
        } else if let KotlinType::TypeParam(ref target_param) = substituted
            && let Some(concrete) = self.bindings.get(target_param).cloned()
        {
            self.bindings
                .insert(constraint.type_param.clone(), concrete);
            return true;
        }
        false
    }

    pub fn solve_in_place(&mut self) -> &AHashMap<SmolStr, KotlinType> {
        let mut changed = true;
        let mut iterations = 0;
        while changed && iterations < 5 {
            changed = false;
            iterations += 1;
            for constraint in &self.constraints.clone() {
                if constraint.is_upper && self.apply_single_upper_constraint(constraint) {
                    changed = true;
                }
            }
        }
        &self.bindings
    }

    pub fn solve(mut self) -> AHashMap<SmolStr, KotlinType> {
        self.solve_in_place();
        self.bindings
    }

    pub fn solve_and_substitute(&mut self, target: &KotlinType) -> KotlinType {
        self.solve_in_place();
        target.substitute(&self.bindings)
    }
}
