use tower_lsp::lsp_types::{Position, SymbolKind, Url};
use tree_sitter::Node;

use crate::indexer::WorkspaceIndex;
pub use crate::inference::{
    infer_receiver_type, infer_receiver_type_depth, resolve_full_type_name, resolve_member_type,
};

/// Resolve receiver AST node to target type/class name directly without string serialization
pub fn resolve_receiver_node_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver_node: Node,
    source: &str,
    root: Option<Node>,
) -> Option<String> {
    crate::inference::infer_node_type(index, current_url, receiver_node, source, root)
        .map(|t| resolve_full_type_name(index, current_url, t.base_name()))
}

/// Resolve receiver expression to target type/class name (delegates to inference module)
pub fn resolve_receiver_type(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Option<String> {
    if let Some(r) = root
        && let Some(node) =
            crate::inference::ast_match::find_matching_ast_node(r, pos, receiver, source)
        && let Some(res) = resolve_receiver_node_type(index, current_url, node, source, root)
    {
        return Some(res);
    }
    infer_receiver_type(index, current_url, receiver, pos, source, root)
        .map(|t| resolve_full_type_name(index, current_url, t.base_name()))
}

pub fn resolve_receiver_type_depth(
    index: &WorkspaceIndex,
    current_url: &Url,
    receiver: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<String> {
    infer_receiver_type_depth(index, current_url, receiver, pos, source, root, depth)
        .map(|t| resolve_full_type_name(index, current_url, t.base_name()))
}

/// Collect candidate container names for member lookups (including supertypes, companions, and nested objects)
pub fn collect_type_containers(index: &WorkspaceIndex, type_name: &str) -> Vec<String> {
    let clean_type = type_name.split('<').next().unwrap_or(type_name).trim();
    let mut containers = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add_candidate = |cand: String| {
        if !cand.is_empty() && seen.insert(cand.clone()) {
            containers.push(cand);
        }
    };

    add_candidate(clean_type.to_string());
    add_candidate(crate::concat_str!(clean_type, ".Companion"));

    let package_prefix = clean_type.rsplit_once('.').map(|(pkg, _)| pkg);
    for super_type in index.get_all_supertypes(clean_type) {
        let clean_super = super_type.split('<').next().unwrap_or(&super_type).trim();
        if let Some(prefix) = package_prefix {
            if clean_super.contains('.') {
                add_candidate(clean_super.to_string());
                add_candidate(crate::concat_str!(clean_super, ".Companion"));
            } else {
                add_candidate(crate::concat_str!(prefix, ".", clean_super));
                add_candidate(crate::concat_str!(prefix, ".", clean_super, ".Companion"));
                add_candidate(clean_super.to_string());
                add_candidate(crate::concat_str!(clean_super, ".Companion"));
            }
        } else {
            add_candidate(clean_super.to_string());
            add_candidate(crate::concat_str!(clean_super, ".Companion"));
        }
    }

    if let Some(primitive) = crate::inference::model::PrimitiveType::from_name(clean_type) {
        for std_interface in &[
            crate::inference::types::StandardInterface::CharSequence,
            crate::inference::types::StandardInterface::Comparable,
            crate::inference::types::StandardInterface::Number,
            crate::inference::types::StandardInterface::Serializable,
            crate::inference::types::StandardInterface::Any,
        ] {
            if std_interface.conforms_primitive(primitive) {
                let std_name = std_interface.as_str();
                add_candidate(std_name.to_string());
                add_candidate(crate::concat_str!("kotlin.", std_name));
            }
        }
    }

    if let Some(decls) = index.container_to_decls.get(clean_type) {
        for (_, decl) in decls {
            if decl.kind == SymbolKind::OBJECT {
                add_candidate(crate::concat_str!(clean_type, ".", decl.name));
            }
        }
    }

    containers
}
