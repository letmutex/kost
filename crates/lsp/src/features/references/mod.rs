mod candidate;
mod conformance;
mod filter;
mod hierarchy;
mod local;
mod overload;
mod receiver_filter;
mod search;
mod target;
mod verify;

use tower_lsp::lsp_types::{Location, Position, Url};
use tree_sitter::Node;

pub use filter::{contains_identifier_token, find_producer_names, is_file_candidate};
pub use hierarchy::conforms_to_container_or_companion;
pub use search::{deduplicate_locations, find_local_references, find_workspace_references};
pub use target::{
    ReferenceTarget, is_declaration_name_node, is_position_in_range, resolve_reference_target,
};

use crate::indexer::WorkspaceIndex;
use crate::parser::parse_source;

pub struct ReferenceQuery<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub symbol_name: &'a str,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
    pub include_declaration: bool,
}

pub fn find_references(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let tree = parse_source(source);
    find_references_with_root(
        index,
        current_url,
        symbol_name,
        pos,
        source,
        tree.as_ref().map(|parsed_tree| parsed_tree.root_node()),
        include_declaration,
    )
}

pub fn find_references_with_root(
    index: &WorkspaceIndex,
    current_url: &Url,
    symbol_name: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    include_declaration: bool,
) -> Vec<Location> {
    find_references_query(ReferenceQuery {
        index,
        current_url,
        symbol_name,
        pos,
        source,
        root,
        include_declaration,
    })
}

pub fn find_references_query(query: ReferenceQuery<'_>) -> Vec<Location> {
    let Some(target) = resolve_reference_target(&query) else {
        return Vec::new();
    };

    let locations = if target.is_local {
        find_local_references(&query, &target)
    } else {
        find_workspace_references(&query, &target)
    };

    deduplicate_locations(locations)
}
