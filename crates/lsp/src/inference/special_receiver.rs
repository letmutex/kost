use tree_sitter::Node;

use super::context::InferenceContext;
use super::scope_function::resolve_scope_keyword_receiver;
use super::types::KotlinType;

fn find_as_node(root: Node) -> Option<Node> {
    let mut current = super::ast_match::find_root_expression_node(root)?;
    while current.kind() == "parenthesized_expression" {
        let mut cursor = current.walk();
        current = current.children(&mut cursor).find(|n| {
            n.is_named() && !matches!(n.kind(), "comment" | "line_comment" | "multiline_comment")
        })?;
    }
    (current.kind() == "as_expression").then_some(current)
}

pub fn resolve_special_expression_receiver(
    ctx: InferenceContext<'_, '_, '_>,
    clean: &str,
) -> Option<KotlinType> {
    if clean.split_whitespace().any(|tok| {
        tok.strip_prefix("as")
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['?', '(', '<', '@']))
    }) && let Some(tree) = crate::parser::parse_source(clean)
        && let Some(as_node) = find_as_node(tree.root_node())
        && let Some(cast_type) =
            super::ast_binary::infer_as_cast(ctx.index, ctx.current_url, as_node, clean)
    {
        ctx.tracer.record_step(|| {
            crate::concat_str!("Cast target resolved to `", &cast_type.to_string(), "`")
        });
        return Some(cast_type);
    }
    if let Some(res) = resolve_scope_keyword_receiver(
        ctx.index,
        ctx.current_url,
        clean,
        ctx.pos,
        ctx.source,
        ctx.root,
        ctx.depth,
    ) {
        ctx.tracer
            .record_step(|| crate::concat_str!("Scope keyword `", clean, "`"));
        return Some(res);
    }
    None
}
