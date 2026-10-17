use tower_lsp::lsp_types::{Location, Url};
use tree_sitter::Node;

use crate::position::node_to_range;
pub use crate::syntax::scope::BindingRole;
use crate::syntax::symbol_item::find_identifier_node;

fn is_file_or_class_member_declaration(node: &Node) -> bool {
    let mut curr = *node;
    while let Some(parent) = curr.parent() {
        if matches!(
            parent.kind(),
            "function_declaration"
                | "secondary_constructor"
                | "primary_constructor"
                | "anonymous_initializer"
                | "lambda_literal"
        ) {
            return false;
        }
        if matches!(parent.kind(), "source_file" | "class_body") {
            return true;
        }
        curr = parent;
    }
    false
}

pub fn find_local_binding_node<'a>(
    node: Node<'a>,
    symbol_name: &str,
    source: &'a str,
    role: BindingRole,
) -> Option<Node<'a>> {
    let found = crate::syntax::scope::find_lexical_binding_node(node, symbol_name, source, role)?;
    if is_file_or_class_member_declaration(&found) {
        return None;
    }
    Some(found)
}

pub fn find_local_binding(
    node: Node,
    symbol_name: &str,
    source: &str,
    current_url: &Url,
    role: BindingRole,
) -> Option<Location> {
    let found_node = find_local_binding_node(node, symbol_name, source, role)?;
    let id_range = find_identifier_node(&found_node)
        .map(|node| node_to_range(&node))
        .unwrap_or_else(|| node_to_range(&found_node));
    Some(Location {
        uri: current_url.clone(),
        range: id_range,
    })
}
