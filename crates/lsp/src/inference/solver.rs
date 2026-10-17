use ahash::AHashMap;
use smol_str::SmolStr;

use super::types::KotlinType;
use super::unify::{unify_primitive_and_class, unify_two_types};
use crate::indexer::WorkspaceIndex;

#[derive(Debug, Clone)]
pub struct TypeConstraint {
    pub type_param: SmolStr,
    pub bound: KotlinType,
    pub is_upper: bool,
}

pub struct ConstraintSystem<'a> {
    pub index: &'a WorkspaceIndex,
    pub type_params: Vec<SmolStr>,
    pub constraints: Vec<TypeConstraint>,
    pub bindings: AHashMap<SmolStr, KotlinType>,
    pub visited_classes: Vec<(SmolStr, SmolStr)>,
}

impl<'a> ConstraintSystem<'a> {
    pub fn new(index: &'a WorkspaceIndex, type_params: Vec<SmolStr>) -> Self {
        Self {
            index,
            type_params,
            constraints: Vec::new(),
            bindings: AHashMap::default(),
            visited_classes: Vec::new(),
        }
    }

    pub fn from_signature(index: &'a WorkspaceIndex, sig: &str) -> Self {
        if let Some(info) = super::signature::SignatureInfo::parse(sig) {
            Self::from_signature_info(index, &info)
        } else {
            let params = super::generics::extract_generic_type_params(sig);
            Self::new(index, params.into_iter().map(SmolStr::new).collect())
        }
    }

    pub fn from_signature_info(
        index: &'a WorkspaceIndex,
        sig_info: &super::signature::SignatureInfo,
    ) -> Self {
        let params: Vec<SmolStr> = sig_info
            .type_params
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let mut system = Self::new(index, params);
        for p in &sig_info.type_params {
            for bound_ty in &p.bounds {
                system.add_upper_bound(p.name.clone(), bound_ty.clone());
            }
        }
        system
    }

    pub fn add_upper_bound(&mut self, param: impl Into<SmolStr>, bound: KotlinType) {
        self.constraints.push(TypeConstraint {
            type_param: param.into(),
            bound,
            is_upper: true,
        });
    }

    pub fn parse_with_params(&self, sig: &str) -> Option<KotlinType> {
        let params: Vec<&str> = self.type_params.iter().map(|s| s.as_str()).collect();
        super::types::parse_type_signature_with_params(sig, &params)
    }

    pub fn bind(&mut self, param: impl Into<SmolStr>, ty: KotlinType) {
        let p = param.into();
        if !self.type_params.contains(&p) {
            self.type_params.push(p.clone());
        }
        self.bindings.insert(p, ty);
    }

    pub fn add_container_type_params_typed(&mut self, receiver_type: &KotlinType) {
        for (param, bound_type) in
            super::generics::extract_container_type_param_bindings_typed(self.index, receiver_type)
        {
            self.bind(param, bound_type);
        }
    }

    pub fn add_container_type_params(&mut self, receiver_type_str: &str) {
        for (param, ty) in
            super::generics::extract_container_type_param_bindings(self.index, receiver_type_str)
        {
            self.bind(param, ty);
        }
    }

    pub fn add_receiver_constraint_typed(
        &mut self,
        actual_receiver: &KotlinType,
        decl_receiver: &KotlinType,
    ) -> bool {
        self.add_subtype_constraint(actual_receiver, decl_receiver)
    }

    pub fn add_receiver_constraint(&mut self, sig: &str, receiver_type_str: &str) -> bool {
        let Some(actual_type) = super::types::parse_type_signature(receiver_type_str) else {
            return false;
        };
        self.add_receiver_constraint_from_sig(sig, &actual_type)
    }

    pub fn add_receiver_constraint_from_sig(
        &mut self,
        sig: &str,
        actual_receiver: &KotlinType,
    ) -> bool {
        let decl_receiver = crate::syntax::signature::extract_receiver_type_from_signature(sig);
        if let Some(expected_type_str) = decl_receiver
            && let Some(expected_type) = self.parse_with_params(&expected_type_str)
        {
            return self.add_subtype_constraint(actual_receiver, &expected_type);
        }
        false
    }

    pub fn add_lambda_return_constraint_typed(
        &mut self,
        param_type: &KotlinType,
        actual_lambda_return: &KotlinType,
    ) -> bool {
        let clean_param_type = param_type.unwrap_nullable_ref();
        if let KotlinType::Function { return_type, .. } = clean_param_type {
            if return_type.as_ref() == &KotlinType::UNIT {
                return true;
            }
            self.add_subtype_constraint(actual_lambda_return, return_type.as_ref())
        } else {
            super::types::is_functional_or_sam(clean_param_type, self.index)
        }
    }

    pub fn add_lambda_return_constraint(
        &mut self,
        sig: &str,
        lambda_return_str: &str,
        arg_idx: usize,
    ) -> bool {
        let (Some(param_type_str), Some(actual_type)) = (
            crate::signature::extract_param_type_by_index(sig, arg_idx),
            super::types::parse_type_signature(lambda_return_str),
        ) else {
            return false;
        };
        if let Some(expected_type) = self.parse_with_params(&param_type_str) {
            self.add_lambda_return_constraint_typed(&expected_type, &actual_type)
        } else if let Some(ret_str) =
            super::generics::extract_functional_return_type(&param_type_str)
        {
            if let Some(expected_ret) = self.parse_with_params(&ret_str) {
                if expected_ret == KotlinType::UNIT {
                    return true;
                }
                self.add_subtype_constraint(&actual_type, &expected_ret)
            } else {
                false
            }
        } else {
            false
        }
    }

    fn as_type_param<'s>(&self, ty: &'s KotlinType) -> Option<&'s SmolStr> {
        match ty {
            KotlinType::TypeParam(name) if self.type_params.contains(name) => Some(name),
            KotlinType::Class { name, args }
                if args.is_empty() && self.type_params.contains(name) =>
            {
                Some(name)
            }
            _ => None,
        }
    }

    pub fn add_subtype_constraint(&mut self, sub: &KotlinType, sup: &KotlinType) -> bool {
        if let (KotlinType::Nothing, _) | (_, KotlinType::DynamicOrAny) = (sub, sup) {
            return true;
        }

        if let Some(name) = self.as_type_param(sup).cloned() {
            return self.record_binding(&name, sub.clone());
        }
        if let Some(name) = self.as_type_param(sub).cloned() {
            return self.record_binding(&name, sup.clone());
        }

        if sup.is_any() {
            return sup.is_nullable() || !sub.is_nullable();
        }

        match (sub, sup) {
            (KotlinType::Nullable(sub_inner), KotlinType::Nullable(sup_inner)) => {
                self.add_subtype_constraint(sub_inner, sup_inner)
            }
            (sub_type, KotlinType::Nullable(sup_inner)) => {
                self.add_subtype_constraint(sub_type, sup_inner)
            }
            (KotlinType::Nullable(_), non_null_sup) => {
                if let Some(name) = self.as_type_param(non_null_sup).cloned() {
                    return self.record_binding(&name, sub.clone());
                }
                false
            }

            // Function types: params contravariant, return covariant, receiver contravariant
            (
                KotlinType::Function {
                    receiver: r1,
                    params: p1,
                    return_type: ret1,
                    ..
                },
                KotlinType::Function {
                    receiver: r2,
                    params: p2,
                    return_type: ret2,
                    ..
                },
            ) => self.unify_function_types(r1.as_deref(), p1, ret1, r2.as_deref(), p2, ret2),

            // Primitive types
            (KotlinType::Primitive(p1), KotlinType::Primitive(p2)) => p1 == p2,

            // Classes with generic arguments
            (
                KotlinType::Class { name: n1, args: a1 },
                KotlinType::Class { name: n2, args: a2 },
            ) => self.unify_class_types(n1, a1, n2, a2),

            // Primitive and Class (e.g. Int is a subtype of Any, Number, Comparable)
            (KotlinType::Primitive(p), KotlinType::Class { name, args }) => {
                unify_primitive_and_class(*p, name, args)
            }

            _ => false,
        }
    }

    fn unify_function_types(
        &mut self,
        r1: Option<&KotlinType>,
        p1: &[KotlinType],
        ret1: &KotlinType,
        r2: Option<&KotlinType>,
        p2: &[KotlinType],
        ret2: &KotlinType,
    ) -> bool {
        if p1.len() != p2.len() {
            return false;
        }
        for (param_sub, param_sup) in p1.iter().zip(p2.iter()) {
            if !self.add_subtype_constraint(param_sup, param_sub) {
                return false;
            }
        }
        if !self.add_subtype_constraint(ret1, ret2) {
            return false;
        }
        match (r1, r2) {
            (Some(recv1), Some(recv2)) => self.add_subtype_constraint(recv2, recv1),
            _ => true,
        }
    }

    fn record_binding(&mut self, param: &SmolStr, ty: KotlinType) -> bool {
        if let Some(existing) = self.bindings.get(param) {
            if existing == &ty {
                return true;
            }
            if existing.unwrap_nullable_ref() == ty.unwrap_nullable_ref() {
                if ty.is_nullable() {
                    self.bindings.insert(param.clone(), ty);
                }
                return true;
            }
            if let Some(lub) = unify_two_types(self.index, existing, &ty) {
                self.bindings.insert(param.clone(), lub);
                return true;
            }
            false
        } else {
            self.bindings.insert(param.clone(), ty);
            true
        }
    }

    pub fn solve_in_place(&mut self) -> &AHashMap<SmolStr, KotlinType> {
        for constraint in &self.constraints {
            if constraint.is_upper {
                let target_ty =
                    if let Some(bound_ty) = self.bindings.get(&constraint.type_param).cloned() {
                        let mut checker = ConstraintSystem::new(self.index, Vec::new());
                        if !checker.add_subtype_constraint(&bound_ty, &constraint.bound) {
                            unify_two_types(self.index, &bound_ty, &constraint.bound)
                                .filter(|u| {
                                    *u != KotlinType::ANY || constraint.bound == KotlinType::ANY
                                })
                                .unwrap_or_else(|| constraint.bound.clone())
                        } else {
                            bound_ty
                        }
                    } else {
                        constraint.bound.clone()
                    };
                self.bindings
                    .insert(constraint.type_param.clone(), target_ty);
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
