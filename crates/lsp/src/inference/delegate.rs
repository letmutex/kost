use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::expression::infer_receiver_type_depth;
use super::member::resolve_member_type_depth;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

/// Extract the delegate expression (the RHS of `by`) for a property declaration node
pub fn extract_delegate_expression<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let prop_node = if node.kind() == "property_declaration" {
        *node
    } else if let Some(parent) = node.parent() {
        if parent.kind() == "property_declaration" {
            parent
        } else {
            *node
        }
    } else {
        *node
    };

    let mut cursor = prop_node.walk();
    for child in prop_node.children(&mut cursor) {
        if child.kind() == "property_delegate" {
            let mut del_cursor = child.walk();
            for del_child in child.children(&mut del_cursor) {
                if del_child.kind() != "by"
                    && del_child.kind() != "comment"
                    && let Some(text) = source.get(del_child.start_byte()..del_child.end_byte())
                {
                    let clean = text.trim();
                    if !clean.is_empty() {
                        return Some(clean);
                    }
                }
            }
            if let Some(text) = source.get(child.start_byte()..child.end_byte()) {
                let trimmed = text.trim();
                if let Some(rest) = trimmed.strip_prefix("by") {
                    let clean = rest.trim();
                    if !clean.is_empty() {
                        return Some(clean);
                    }
                }
            }
        }
    }
    None
}

/// Infer the type of a delegated property by resolving `getValue` on the delegate expression's type
pub fn infer_delegated_property_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    node: &Node,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if depth > super::MAX_INFERENCE_DEPTH {
        return None;
    }
    let delegate_expr = extract_delegate_expression(node, source)?;
    let delegate_type = infer_receiver_type_depth(
        index,
        current_url,
        delegate_expr,
        pos,
        source,
        root,
        depth + 1,
    )?;

    let non_null_del = delegate_type.make_non_null();
    let mut tracer = super::trace::Tracer::Noop;
    let ctx = super::InferenceContext::new(
        index,
        current_url,
        pos,
        source,
        root,
        depth + 1,
        &mut tracer,
    );
    resolve_member_type_depth(ctx, &non_null_del.to_string(), "getValue")
}

/// Infer property type from delegate expression by analyzing source file
pub fn resolve_property_delegate_from_source(
    index: &WorkspaceIndex,
    url: &Url,
    decl: &crate::indexer::DeclarationItem,
    decl_source: &str,
    depth: usize,
) -> Option<KotlinType> {
    if decl.kind != tower_lsp::lsp_types::SymbolKind::PROPERTY {
        return None;
    }
    let tree = crate::parser::parse_source(decl_source)?;
    let root_node = tree.root_node();
    let id_node = crate::parser::find_node_at_position(root_node, decl.selection_range.start)?;
    let prop_node = crate::parser::find_enclosing_declaration_node(id_node);
    infer_delegated_property_type(
        index,
        url,
        &prop_node,
        decl.selection_range.start,
        decl_source,
        Some(root_node),
        depth + 1,
    )
}
