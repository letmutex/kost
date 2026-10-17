pub use super::primitive::clean_variance;
use super::primitive::{get_primitive_type, is_primitive_conforming};
use crate::indexer::WorkspaceIndex;
use crate::inference::signature::{GenericParamInfo, SignatureInfo};
use crate::inference::supertypes::get_generic_supertypes;
use crate::inference::types::standard::StandardInterface;
use crate::inference::types::{KotlinType, PrimitiveType, TypeArg, Variance};

fn instantiate_super_args_algebraic(
    sub_name: &str,
    sub_args: &[TypeArg],
    super_ty: &KotlinType,
    index: &WorkspaceIndex,
) -> KotlinType {
    if sub_args.is_empty() {
        return super_ty.clone();
    }
    let Some(decls) = index.name_to_decls.get(sub_name) else {
        return super_ty.clone();
    };
    for (_, d) in decls {
        if let Some(sig_info) = SignatureInfo::from_decl(d)
            && !sig_info.type_params.is_empty()
        {
            let mut bindings = ahash::AHashMap::default();
            for (tp, arg) in sig_info.type_params.iter().zip(sub_args.iter()) {
                bindings.insert(tp.name.clone(), arg.type_ref.clone());
            }
            return super_ty.substitute(&bindings);
        }
    }
    super_ty.clone()
}

struct VisitedGuard<'a>(&'a mut Vec<(smol_str::SmolStr, smol_str::SmolStr)>);

impl<'a> VisitedGuard<'a> {
    fn push(
        visited: &'a mut Vec<(smol_str::SmolStr, smol_str::SmolStr)>,
        a: &str,
        b: &str,
    ) -> Self {
        visited.push((smol_str::SmolStr::new(a), smol_str::SmolStr::new(b)));
        Self(visited)
    }

    fn vec_mut(&mut self) -> &mut Vec<(smol_str::SmolStr, smol_str::SmolStr)> {
        self.0
    }
}

impl<'a> Drop for VisitedGuard<'a> {
    fn drop(&mut self) {
        self.0.pop();
    }
}

struct SubtypeCtx<'a> {
    index: &'a WorkspaceIndex,
    type_params: &'a [GenericParamInfo],
    depth: usize,
    visited: &'a mut Vec<(smol_str::SmolStr, smol_str::SmolStr)>,
}

fn check_class_args_subtype(
    act_args: &[TypeArg],
    bnd_args: &[TypeArg],
    bound_short: &str,
    ctx: &mut SubtypeCtx<'_>,
) -> bool {
    if bnd_args.is_empty() {
        return true;
    }
    if act_args.len() != bnd_args.len() {
        return false;
    }
    let is_cov = StandardInterface::from_name(bound_short).is_some_and(|i| i.is_collection());
    let mut deeper = SubtypeCtx {
        index: ctx.index,
        type_params: ctx.type_params,
        depth: ctx.depth + 1,
        visited: ctx.visited,
    };
    act_args.iter().zip(bnd_args.iter()).all(|(a_arg, b_arg)| {
        let v = if is_cov || b_arg.variance == Variance::Out {
            Variance::Out
        } else {
            b_arg.variance
        };
        match v {
            Variance::Star => true,
            Variance::Out => is_subtype_depth(&a_arg.type_ref, &b_arg.type_ref, &mut deeper),
            Variance::In => is_subtype_depth(&b_arg.type_ref, &a_arg.type_ref, &mut deeper),
            Variance::Invariant => {
                a_arg.type_ref == b_arg.type_ref
                    || (is_subtype_depth(&a_arg.type_ref, &b_arg.type_ref, &mut deeper)
                        && is_subtype_depth(&b_arg.type_ref, &a_arg.type_ref, &mut deeper))
            }
        }
    })
}

fn check_index_supertypes(
    actual_base: &str,
    actual_short: &str,
    bound_base: &str,
    bound_short: &str,
    act_args: &[TypeArg],
    bound_u: &KotlinType,
    ctx: &mut SubtypeCtx<'_>,
) -> bool {
    for super_ty in get_generic_supertypes(ctx.index, actual_short) {
        let super_base = super_ty.base_name();
        let super_short = super_base.rsplit('.').next().unwrap_or(super_base);
        if super_short == actual_short || super_base == actual_base {
            continue;
        }
        let inst = instantiate_super_args_algebraic(actual_short, act_args, &super_ty, ctx.index);
        let mut deeper = SubtypeCtx {
            index: ctx.index,
            type_params: ctx.type_params,
            depth: ctx.depth + 1,
            visited: ctx.visited,
        };
        if is_subtype_depth(&inst, bound_u, &mut deeper) {
            return true;
        }
    }

    let matches_bound = |s: &str| {
        let sb = s.split('<').next().unwrap_or(s).trim();
        let ss = sb.rsplit('.').next().unwrap_or(sb);
        sb == bound_base || ss == bound_short
    };

    if ctx
        .index
        .get_all_supertypes(actual_base)
        .iter()
        .any(|s| matches_bound(s))
    {
        return true;
    }

    if actual_short != actual_base
        && ctx
            .index
            .get_all_supertypes(actual_short)
            .iter()
            .any(|s| matches_bound(s))
    {
        return true;
    }

    false
}

pub fn is_subtype(
    actual: &KotlinType,
    bound: &KotlinType,
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> bool {
    let mut visited = Vec::new();
    let mut ctx = SubtypeCtx {
        index,
        type_params,
        depth: 0,
        visited: &mut visited,
    };
    is_subtype_depth(actual, bound, &mut ctx)
}

fn is_trivial_subtype(actual_u: &KotlinType, bound_u: &KotlinType) -> Option<bool> {
    if actual_u == bound_u {
        return Some(true);
    }
    if matches!(
        actual_u,
        KotlinType::Nothing | KotlinType::Primitive(PrimitiveType::Nothing)
    ) {
        return Some(true);
    }
    if bound_u.is_any() || matches!(bound_u, KotlinType::DynamicOrAny) {
        return Some(true);
    }
    if let KotlinType::Class { name, .. } = bound_u
        && (name == "Object" || name == "java.lang.Object")
    {
        return Some(true);
    }
    None
}

fn check_type_param_bounds(
    actual_short: &str,
    bound_u: &KotlinType,
    ctx: &mut SubtypeCtx<'_>,
) -> Option<bool> {
    let param_info = ctx.type_params.iter().find(|p| p.name == actual_short)?;
    let mut deeper = SubtypeCtx {
        index: ctx.index,
        type_params: ctx.type_params,
        depth: ctx.depth + 1,
        visited: ctx.visited,
    };
    Some(
        param_info.bounds.is_empty() && bound_u.is_any()
            || param_info
                .bounds
                .iter()
                .any(|b| is_subtype_depth(b, bound_u, &mut deeper)),
    )
}

fn check_class_and_supertypes(
    actual_u: &KotlinType,
    bound_u: &KotlinType,
    actual_base: &str,
    actual_short: &str,
    bound_base: &str,
    bound_short: &str,
    ctx: &mut SubtypeCtx<'_>,
) -> bool {
    if let Some(prim) = get_primitive_type(actual_u)
        && is_primitive_conforming(prim, bound_u, ctx.type_params)
    {
        return true;
    }

    let act_args = actual_u.type_arguments();
    let bnd_args = bound_u.type_arguments();
    if (actual_base == bound_base || actual_short == bound_short)
        && check_class_args_subtype(act_args, bnd_args, bound_short, ctx)
    {
        return true;
    }

    check_index_supertypes(
        actual_base,
        actual_short,
        bound_base,
        bound_short,
        act_args,
        bound_u,
        ctx,
    )
}

fn is_subtype_depth(actual: &KotlinType, bound: &KotlinType, ctx: &mut SubtypeCtx<'_>) -> bool {
    if ctx.depth > crate::inference::MAX_INFERENCE_DEPTH {
        return false;
    }
    if actual.is_nullable() && !bound.is_nullable() {
        return false;
    }

    let actual_u = actual.unwrap_nullable_ref();
    let bound_u = bound.unwrap_nullable_ref();

    if let Some(res) = is_trivial_subtype(actual_u, bound_u) {
        return res;
    }

    let actual_base = actual_u.base_name();
    let bound_base = bound_u.base_name();
    if ctx
        .visited
        .iter()
        .any(|(a, b)| a.as_str() == actual_base && b.as_str() == bound_base)
    {
        return false;
    }

    let mut guard = VisitedGuard::push(ctx.visited, actual_base, bound_base);

    let actual_short = actual_base.rsplit('.').next().unwrap_or(actual_base);
    let mut sub_ctx = SubtypeCtx {
        index: ctx.index,
        type_params: ctx.type_params,
        depth: ctx.depth,
        visited: guard.vec_mut(),
    };
    if let Some(res) = check_type_param_bounds(actual_short, bound_u, &mut sub_ctx) {
        return res;
    }

    let bound_short = bound_base.rsplit('.').next().unwrap_or(bound_base);
    if sub_ctx.type_params.iter().any(|p| p.name == bound_short) {
        return actual_short == bound_short;
    }

    check_class_and_supertypes(
        actual_u,
        bound_u,
        actual_base,
        actual_short,
        bound_base,
        bound_short,
        &mut sub_ctx,
    )
}

pub fn satisfies_bound(
    actual: &str,
    bound: &str,
    index: &WorkspaceIndex,
    type_params: &[GenericParamInfo],
) -> bool {
    let Some(actual_ty) = crate::inference::types::parse_type_signature(actual) else {
        return false;
    };
    let Some(bound_ty) = crate::inference::types::parse_type_signature(bound) else {
        return false;
    };
    is_subtype(&actual_ty, &bound_ty, index, type_params)
}
