use tower_lsp::lsp_types::{Position, Url};

use super::call::{CallInfo, parse_call_expression};
use super::context::InferenceContext;
use super::member_container::resolve_container_members;
use super::member_extension::{infer_call_argument_types, resolve_extension_members};
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

/// Resolve member type of a container (convenience function with default dummy URL)
pub fn resolve_member_type(
    index: &WorkspaceIndex,
    container_type: &str,
    member_expr: &str,
) -> Option<KotlinType> {
    let dummy_url = Url::parse("file:///project/Dummy.kt").ok()?;
    let mut tracer = Tracer::Noop;
    let ctx = InferenceContext::new(
        index,
        &dummy_url,
        Position::default(),
        "",
        None,
        0,
        &mut tracer,
    );
    resolve_member_type_depth(ctx, container_type, member_expr)
}

/// Resolve member type with recursion depth guard
pub fn resolve_member_type_depth(
    ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    member_expr: &str,
) -> Option<KotlinType> {
    resolve_member_type_depth_traced(ctx, container_type, member_expr)
}

/// Resolve member type for a KotlinType container with depth guard and tracing
pub fn resolve_member_type_kt_depth_traced(
    mut ctx: InferenceContext<'_, '_, '_>,
    container_type: &KotlinType,
    member_expr: &str,
) -> Option<KotlinType> {
    let clean = member_expr.trim();
    if super::lambda_call::is_call_with_lambda(clean) {
        let container_str = container_type.to_string();
        let target = crate::concat_str!(clean, " on ", container_str);
        ctx.tracer.enter_scope("MemberCall", &target);
        let res = super::lambda_call::resolve_higher_order_call(
            ctx.reborrow(),
            Some(container_type),
            clean,
        );
        if let Some(ref kt) = res {
            ctx.tracer.exit_scope(Some(&kt.to_string()));
        } else {
            ctx.tracer.exit_scope(None);
        }
        if res.is_some() {
            return res;
        }
    }

    let call_info = parse_call_expression(clean);
    let inferred_arg_types = infer_call_argument_types(
        ctx.index,
        ctx.current_url,
        &call_info,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    );

    resolve_member_call_info_depth_traced(
        ctx,
        container_type,
        clean,
        &call_info,
        &inferred_arg_types,
    )
}

/// Resolve member type for a pre-parsed CallInfo and pre-inferred argument types
pub fn resolve_member_call_info_depth_traced(
    mut ctx: InferenceContext<'_, '_, '_>,
    container_type: &KotlinType,
    member_expr: &str,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) -> Option<KotlinType> {
    let container_str = container_type.to_string();
    let target = crate::concat_str!(member_expr, " on ", container_str);
    ctx.tracer.enter_scope("MemberCall", &target);

    record_evaluated_arguments(ctx.tracer, call_info, inferred_arg_types);

    let res = resolve_container_members(
        ctx.reborrow(),
        &container_str,
        call_info,
        inferred_arg_types,
    )
    .or_else(|| {
        resolve_extension_members(
            ctx.reborrow(),
            &container_str,
            call_info,
            inferred_arg_types,
        )
    });

    if let Some(ref kt) = res {
        ctx.tracer.exit_scope(Some(&kt.to_string()));
    } else {
        ctx.tracer.exit_scope(None);
    }
    res
}

pub fn resolve_member_type_depth_traced(
    ctx: InferenceContext<'_, '_, '_>,
    container_type: &str,
    member_expr: &str,
) -> Option<KotlinType> {
    let parsed_type = super::types::parse_type_signature(container_type)
        .unwrap_or_else(|| KotlinType::simple_class(container_type));
    resolve_member_type_kt_depth_traced(ctx, &parsed_type, member_expr)
}

fn record_evaluated_arguments(
    tracer: &mut Tracer,
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
) {
    if !call_info.value_args.is_empty() {
        tracer.record_step(|| {
            let formatted: Vec<String> = call_info
                .value_args
                .iter()
                .zip(inferred_arg_types.iter())
                .map(|(arg, ty)| {
                    let ty_str = ty
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    crate::concat_str!("`", arg, "` -> `", ty_str, "`")
                })
                .collect();
            crate::concat_str!("Evaluated arguments: [", formatted.join(", "), "]")
        });
    }
}
