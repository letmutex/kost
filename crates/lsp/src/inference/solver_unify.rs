use ahash::AHashMap;
use smol_str::SmolStr;

use super::solver::ConstraintSystem;
use super::types::{KotlinType, TypeArg, Variance};
use super::unify::is_class_subtype;

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
            let is_cov = matches!(
                n2_s,
                "List" | "Iterable" | "Collection" | "Set" | "Sequence"
            );
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
