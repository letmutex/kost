use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
use super::expression::resolve_full_type_name;
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;
use crate::parser::{extract_instantiated_type, extract_type_annotation, find_node_at_position};
pub use crate::syntax::ast_util::{extract_initializer_expression, extract_initializer_node};

/// Infer the type of a local variable or parameter at the given position
pub fn infer_variable_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    var_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    infer_variable_type_traced(
        index,
        current_url,
        var_name,
        pos,
        source,
        root,
        &mut Tracer::Noop,
    )
}

pub fn infer_variable_type_traced(
    index: &WorkspaceIndex,
    current_url: &Url,
    var_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    tracer: &mut Tracer,
) -> Option<KotlinType> {
    tracer.enter_scope("Variable", var_name);
    let tree = root
        .is_none()
        .then(|| crate::parser::parse_source(source))
        .flatten();
    let root_node = root.or_else(|| tree.as_ref().map(|t| t.root_node()));
    let res = resolve_local_receiver_traced(
        index,
        current_url,
        var_name,
        pos,
        source,
        root_node,
        0,
        tracer,
    );
    if let Some(ref kt) = res {
        tracer.exit_scope(Some(&kt.to_string()));
    } else {
        tracer.exit_scope(None);
    }
    res
}

/// Resolve local variable or parameter type with recursion depth guard
pub fn resolve_local_receiver(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    resolve_local_receiver_traced(
        index,
        current_url,
        clean,
        pos,
        source,
        root,
        depth,
        &mut Tracer::Noop,
    )
}

pub fn resolve_local_receiver_kt_traced(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
    tracer: &mut Tracer,
) -> Option<KotlinType> {
    resolve_local_receiver_traced(index, current_url, clean, pos, source, root, depth, tracer)
}

pub fn resolve_local_receiver_traced(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
    tracer: &mut Tracer,
) -> Option<KotlinType> {
    let offset = crate::position::position_to_byte_offset(pos, source);
    let target_node = root.and_then(|r| {
        r.named_descendant_for_byte_range(offset, offset)
            .or_else(|| {
                (offset > 0)
                    .then(|| r.named_descendant_for_byte_range(offset - 1, offset - 1))
                    .flatten()
            })
            .or_else(|| find_node_at_position(r, pos))
    })?;
    let found_node = crate::syntax::scope::find_lexical_binding_node_before(
        target_node,
        clean,
        source,
        offset,
        crate::syntax::scope::BindingRole::Value,
    )
    .or_else(|| {
        crate::syntax::scope::find_lexical_binding_node_before(
            target_node,
            clean,
            source,
            usize::MAX,
            crate::syntax::scope::BindingRole::Value,
        )
    })?;
    let decl_node = if found_node.kind() == "variable_declaration"
        && found_node
            .parent()
            .is_some_and(|p| p.kind() == "property_declaration")
    {
        found_node.parent().unwrap()
    } else {
        found_node
    };
    tracer.record_step(|| common::concat_str!("Found local declaration `", clean, "`"));
    let ctx = InferenceContext::new(index, current_url, pos, source, root, depth, tracer);
    infer_decl_node_type(&decl_node, clean, ctx)
}

fn infer_initializer_type(
    decl_node: &Node,
    init_expr: &str,
    mut ctx: InferenceContext<'_, '_, '_>,
) -> Option<KotlinType> {
    let expected = extract_type_annotation(decl_node, ctx.source).and_then(|t| {
        let raw_type =
            super::typealias::resolve_type_alias_or_import(ctx.index, ctx.current_url, t);
        let full = resolve_full_type_name(ctx.index, ctx.current_url, &raw_type);
        super::types::parse_type_signature(&full)
    });
    if let Some(init_node) = extract_initializer_node(decl_node) {
        let mut eval_ctx =
            super::ast_eval::AstEvalContext::from_ctx(ctx.sub()).with_expected(expected);
        return super::ast_eval::infer_expression_node(&mut eval_ctx, init_node);
    }
    super::expression::infer_receiver_type_expected_traced(ctx.sub(), init_expr, expected.as_ref())
}

fn infer_annotated_decl_type(
    decl_node: &Node,
    source: &str,
    index: &WorkspaceIndex,
    current_url: &Url,
    tracer: &mut Tracer,
) -> Option<KotlinType> {
    let t = extract_type_annotation(decl_node, source)?;
    tracer.record_step(|| common::concat_str!("Explicit type annotation `", t, "`"));
    let raw_type = super::typealias::resolve_type_alias_or_import(index, current_url, t);
    let full = resolve_full_type_name(index, current_url, &raw_type);
    tracer.record_step(|| common::concat_str!("Resolved type to `", &full, "`"));
    super::types::parse_type_signature(&full)
}

fn infer_property_decl_type(
    decl_node: &Node,
    clean: &str,
    mut ctx: InferenceContext<'_, '_, '_>,
) -> Option<KotlinType> {
    if let Some(init_expr) = extract_initializer_expression(decl_node, ctx.source)
        && init_expr != clean
    {
        ctx.tracer
            .record_step(|| common::concat_str!("Evaluating initializer `", init_expr, "`"));
        if let Some(t) = infer_initializer_type(decl_node, init_expr, ctx.reborrow()) {
            return Some(t);
        }
    }
    if super::delegate::extract_delegate_expression(decl_node, ctx.source).is_some() {
        ctx.tracer
            .record_step(|| "Evaluating delegated property expression".to_string());
        return super::delegate::infer_delegated_property_type(
            ctx.index,
            ctx.current_url,
            decl_node,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        );
    }
    if let Some(t) = extract_instantiated_type(decl_node, ctx.source) {
        ctx.tracer
            .record_step(|| common::concat_str!("Found instantiated type `", t, "`"));
        let raw_type =
            super::typealias::resolve_type_alias_or_import(ctx.index, ctx.current_url, t);
        let full = resolve_full_type_name(ctx.index, ctx.current_url, &raw_type);
        return super::types::parse_type_signature(&full);
    }
    None
}

fn infer_decl_node_type(
    decl_node: &Node,
    clean: &str,
    mut ctx: InferenceContext<'_, '_, '_>,
) -> Option<KotlinType> {
    if let Some(t) = infer_annotated_decl_type(
        decl_node,
        ctx.source,
        ctx.index,
        ctx.current_url,
        ctx.tracer,
    ) {
        return Some(t);
    }
    if decl_node
        .parent()
        .is_some_and(|p| p.kind() == "multi_variable_declaration")
        && let Some(t) =
            super::destructuring::infer_destructured_variable_type(ctx.reborrow(), decl_node, clean)
    {
        return Some(t);
    }
    if let Some(t) = infer_property_decl_type(decl_node, clean, ctx.reborrow()) {
        return Some(t);
    }
    if decl_node.kind() != "property_declaration" {
        ctx.tracer
            .record_step(|| "Checking lambda parameter type".to_string());
        if let Some(t) = super::lambda::infer_lambda_param_type(
            ctx.index,
            ctx.current_url,
            *decl_node,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth + 1,
        ) {
            return Some(t);
        }
    }
    None
}
