use super::context::InferenceContext;
use super::signature::SignatureInfo;
use super::solver::ConstraintSystem;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

pub use super::call_parser::{
    CallInfo, parse_call_expression, parse_callee_and_type_args, split_call_arguments,
};

/// Unify multiple concrete types to their least common supertype (or Any if heterogeneous)
pub fn unify_types(index: &WorkspaceIndex, types: &[KotlinType]) -> Option<KotlinType> {
    if types.is_empty() {
        return None;
    }
    let mut current = types[0].clone();
    for t in &types[1..] {
        current = super::unify::unify_two_types(index, &current, t)?;
    }
    Some(current)
}

/// Solve generic return type for a standard function call (explicit type args or inferred from value args)
pub fn solve_call_generic_return_type(
    ctx: InferenceContext<'_, '_, '_>,
    sig: &str,
    decl_ret_type: &str,
    receiver_type_str: Option<&str>,
    call_info: &CallInfo,
) -> KotlinType {
    solve_call_generic_return_type_traced(
        ctx,
        sig,
        decl_ret_type,
        receiver_type_str,
        call_info,
        None,
    )
}

/// Solve generic return type for a candidate declaration using pre-parsed SignatureInfo
pub fn solve_call_decl_return_type_traced(
    ctx: InferenceContext<'_, '_, '_>,
    decl: &crate::indexer::DeclarationItem,
    ret_type: Option<&KotlinType>,
    receiver_type_str: Option<&str>,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> KotlinType {
    let sig = decl.signature.as_deref().unwrap_or("");
    let sig_info = SignatureInfo::from_decl(decl);
    solve_call_generic_return_type_impl(
        ctx,
        sig,
        sig_info.as_ref(),
        ret_type,
        None,
        receiver_type_str,
        call_info,
        inferred_arg_types,
    )
}

/// Solve generic return type with active trace logging
pub fn solve_call_generic_return_type_traced(
    ctx: InferenceContext<'_, '_, '_>,
    sig: &str,
    decl_ret_type: &str,
    receiver_type_str: Option<&str>,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> KotlinType {
    let sig_info = SignatureInfo::parse(sig);
    solve_call_generic_return_type_impl(
        ctx,
        sig,
        sig_info.as_ref(),
        None,
        Some(decl_ret_type),
        receiver_type_str,
        call_info,
        inferred_arg_types,
    )
}

fn resolve_target_return_type(
    system: &ConstraintSystem,
    sig_info: Option<&SignatureInfo>,
    explicit_ret_type: Option<&KotlinType>,
    fallback_ret_type_str: Option<&str>,
) -> KotlinType {
    explicit_ret_type
        .cloned()
        .or_else(|| sig_info.and_then(|info| info.return_type.clone()))
        .or_else(|| fallback_ret_type_str.and_then(|s| system.parse_with_params(s)))
        .or_else(|| fallback_ret_type_str.and_then(super::types::parse_type_signature))
        .unwrap_or_else(|| {
            fallback_ret_type_str
                .map(KotlinType::simple_class)
                .unwrap_or(KotlinType::UNIT)
        })
}

fn try_recover_erased_getter(
    index: &WorkspaceIndex,
    callee: &str,
    substituted: &KotlinType,
    receiver_type_str: Option<&str>,
) -> Option<KotlinType> {
    if substituted.is_any() && (callee == "getValue" || callee == "value") {
        let rec_str = receiver_type_str?;
        let arg = super::generics::recover_erased_generic_arg(index, rec_str)?;
        super::types::parse_type_signature(&arg)
    } else {
        None
    }
}

fn solve_call_generic_return_type_impl(
    mut ctx: InferenceContext<'_, '_, '_>,
    sig: &str,
    sig_info: Option<&SignatureInfo>,
    explicit_ret_type: Option<&KotlinType>,
    fallback_ret_type_str: Option<&str>,
    receiver_type_str: Option<&str>,
    call_info: &CallInfo,
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) -> KotlinType {
    let has_trace = ctx.tracer.is_active();
    if has_trace {
        ctx.tracer.enter_scope("GenericCall", call_info.callee);
    }

    let mut system = if let Some(info) = sig_info {
        ConstraintSystem::from_signature_info(ctx.index, info)
    } else {
        ConstraintSystem::from_signature(ctx.index, sig)
    };

    populate_receiver_and_explicit_type_args(
        &mut system,
        sig,
        receiver_type_str,
        &call_info.explicit_type_args,
    );

    populate_value_arg_constraints(
        &mut system,
        ctx.reborrow(),
        sig,
        sig_info,
        &call_info.value_args,
        inferred_arg_types,
    );

    let target =
        resolve_target_return_type(&system, sig_info, explicit_ret_type, fallback_ret_type_str);
    let substituted = system.solve_and_substitute(&target);

    if let Some(recovered) =
        try_recover_erased_getter(ctx.index, call_info.callee, &substituted, receiver_type_str)
    {
        if has_trace {
            ctx.tracer.exit_scope(Some(&recovered.to_string()));
        }
        return recovered;
    }

    if has_trace {
        for (param, bound) in &system.bindings {
            ctx.tracer
                .record_binding(param.as_str(), &bound.to_string());
        }
        ctx.tracer.exit_scope(Some(&substituted.to_string()));
    }
    substituted
}

fn populate_receiver_and_explicit_type_args(
    system: &mut ConstraintSystem,
    sig: &str,
    receiver_type_str: Option<&str>,
    explicit_type_args: &[String],
) {
    if let Some(rec_str) = receiver_type_str {
        system.add_container_type_params(rec_str);
        system.add_receiver_constraint(sig, rec_str);
    }

    for (param, explicit_arg) in system
        .type_params
        .clone()
        .iter()
        .zip(explicit_type_args.iter())
    {
        let clean_arg = explicit_arg.trim();
        if !clean_arg.is_empty()
            && let Some(k_ty) = super::types::parse_type_signature(clean_arg)
        {
            system.bind(param.clone(), k_ty);
        }
    }
}

fn populate_value_arg_constraints(
    system: &mut ConstraintSystem,
    mut ctx: InferenceContext<'_, '_, '_>,
    sig: &str,
    sig_info: Option<&SignatureInfo>,
    value_args: &[&str],
    inferred_arg_types: Option<&[Option<KotlinType>]>,
) {
    for (idx, arg_expr) in value_args.iter().enumerate() {
        let expected_param = if let Some(info) = sig_info {
            info.params
                .get(idx)
                .or_else(|| info.params.last().filter(|p| p.is_vararg))
                .and_then(|p| system.parse_with_params(&p.type_name))
        } else if let Some(param_str) = crate::signature::extract_param_type_by_index(sig, idx) {
            system.parse_with_params(&param_str)
        } else {
            None
        };

        let arg_type = inferred_arg_types
            .and_then(|args| args.get(idx))
            .and_then(|opt| opt.as_ref())
            .cloned()
            .or_else(|| {
                super::expression::infer_receiver_type_expected_traced(
                    ctx.sub(),
                    arg_expr,
                    expected_param.as_ref(),
                )
            });

        if let Some(arg_type) = arg_type
            && let Some(expected) = expected_param
        {
            system.add_subtype_constraint(&arg_type, &expected);
        }
    }
}
