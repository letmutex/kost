use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::context::InferenceContext;
pub use super::member::{resolve_member_type, resolve_member_type_depth};
pub use super::qualification::{
    qualify_type, resolve_full_type_name, resolve_type_by_imports_or_package,
};
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

/// Infer the type of a receiver expression at a given position
pub fn infer_receiver_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    let tree = root
        .is_none()
        .then(|| crate::parser::parse_source(source))
        .flatten();
    let root_node = root.or_else(|| tree.as_ref().map(|t| t.root_node()));
    infer_receiver_type_depth(index, current_url, receiver, pos, source, root_node, 0)
}

/// Infer the return type of an expression string
pub fn infer_expression_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    expression: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    infer_receiver_type(index, current_url, expression, pos, source, root)
}

/// Infer the return type directly from an existing AST node without string serialization or re-parsing
pub fn infer_node_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    node: Node,
    source: &str,
    root: Option<Node>,
) -> Option<KotlinType> {
    let pos = crate::position::point_to_position(node.start_position());
    let mut tracer = super::trace::Tracer::Noop;
    let ctx = InferenceContext::new(index, current_url, pos, source, root, 0, &mut tracer);
    let mut eval_ctx = super::ast_eval::AstEvalContext::from_ctx(ctx);
    super::ast_eval::infer_expression_node(&mut eval_ctx, node)
}

/// Core recursive inference loop with depth limit
pub fn infer_receiver_type_depth(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    infer_receiver_type_depth_traced(
        index,
        current_url,
        receiver,
        pos,
        source,
        root,
        depth,
        &mut super::trace::Tracer::Noop,
    )
}

pub fn infer_receiver_type_depth_traced(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
    tracer: &mut super::trace::Tracer,
) -> Option<KotlinType> {
    let ctx = InferenceContext::new(index, current_url, pos, source, root, depth, tracer);
    infer_receiver_type_expected_traced(ctx, receiver, None)
}

pub fn infer_receiver_type_expected_traced(
    mut ctx: InferenceContext<'_, '_, '_>,
    receiver: &str,
    expected: Option<&super::types::KotlinType>,
) -> Option<KotlinType> {
    if ctx.depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let clean = crate::syntax::ast_extra::strip_enclosing_parens(receiver);
    if clean.is_empty() {
        return None;
    }

    ctx.tracer.enter_scope("Expression", clean);

    let res: Option<KotlinType> = (|| {
        if let Some(matched) =
            super::qualification::match_qualified_class(ctx.index, clean, ctx.tracer)
        {
            return Some(matched);
        }

        if let Some(inferred) =
            super::ast_eval::try_ast_evaluation_expected(ctx.sub(), clean, expected)
        {
            return Some(inferred);
        }

        resolve_single_receiver(ctx.sub(), clean)
    })();

    if let Some(ref kt) = res {
        ctx.tracer.exit_scope(Some(&kt.to_string()));
    } else {
        ctx.tracer.exit_scope(None);
    }
    res
}

pub(crate) fn resolve_single_receiver(
    mut ctx: InferenceContext<'_, '_, '_>,
    receiver: &str,
) -> Option<KotlinType> {
    let clean = receiver.trim();
    if let Some(t) = resolve_special_or_call_receiver(ctx.reborrow(), clean) {
        return Some(t);
    }
    super::symbol_receiver::resolve_named_symbol_receiver(ctx, clean)
}

fn resolve_special_or_call_receiver(
    mut ctx: InferenceContext<'_, '_, '_>,
    clean: &str,
) -> Option<KotlinType> {
    if let Some(res) =
        super::special_receiver::resolve_special_expression_receiver(ctx.reborrow(), clean)
    {
        return Some(res);
    }
    if super::lambda_call::is_call_with_lambda(clean) {
        ctx.tracer
            .record_step(|| crate::concat_str!("Call with lambda `", clean, "`"));
        if let Some(call_type) =
            super::lambda_call::resolve_higher_order_call(ctx.reborrow(), None, clean)
        {
            return Some(call_type);
        }
    }
    let is_call = clean.ends_with(')')
        && crate::parser::parse_source(clean).is_some_and(|tree| {
            super::ast_match::find_root_expression_node(tree.root_node())
                .is_some_and(|node| node.kind() == "call_expression")
        });
    if is_call
        && let Some(return_type) = super::function::find_function_return_type_depth(
            ctx.index,
            ctx.current_url,
            clean,
            ctx.pos,
            ctx.source,
            ctx.root,
            ctx.depth,
        )
    {
        return Some(return_type);
    }
    let res = super::infix::resolve_infix_expression(
        ctx.index,
        ctx.current_url,
        clean,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    );
    if let Some(ref infix_type) = res {
        ctx.tracer.record_step(|| {
            crate::concat_str!(
                "Infix expression `",
                clean,
                "` resolved to `",
                &infix_type.to_string(),
                "`"
            )
        });
    }
    res
}
