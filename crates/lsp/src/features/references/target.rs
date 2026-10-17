use smol_str::SmolStr;
use tower_lsp::lsp_types::{Location, Position, Range, SymbolKind};
use tree_sitter::Node;

use super::ReferenceQuery;
use super::local::resolve_local_target;
use crate::features::definition::find_definition_with_root;
use crate::indexer::WorkspaceIndex;
use crate::parser::find_node_at_position;
use crate::syntax::symbol_item::find_identifier_node;

#[derive(Debug, Clone)]
pub struct ReferenceTarget {
    pub name: String,
    pub location: Location,
    pub is_local: bool,
    pub local_scope_node_range: Option<Range>,
    pub is_private: bool,
    pub is_extension: bool,
    pub is_override: bool,
    pub kind: Option<SymbolKind>,
    pub container_name: Option<SmolStr>,
    pub signature: Option<SmolStr>,
}

pub fn is_position_in_range(pos: Position, range: Range) -> bool {
    let after_start = pos.line > range.start.line
        || (pos.line == range.start.line && pos.character >= range.start.character);
    let before_end = pos.line < range.end.line
        || (pos.line == range.end.line && pos.character <= range.end.character);
    after_start && before_end
}

fn is_decl_or_container_private(
    file_symbols: &crate::indexer::FileSymbols,
    decl: &crate::indexer::DeclarationItem,
) -> bool {
    if decl.is_private {
        return true;
    }
    if let Some(ref container) = decl.container_name {
        let mut expected_parent: Option<String> = None;
        for segment in container.split('.') {
            let found = file_symbols.declarations.iter().find(|container_decl| {
                container_decl.name == segment
                    && container_decl.container_name.as_deref() == expected_parent.as_deref()
                    && matches!(
                        container_decl.kind,
                        SymbolKind::CLASS | SymbolKind::OBJECT | SymbolKind::INTERFACE
                    )
            });
            if let Some(container_decl) = found
                && container_decl.is_private
            {
                return true;
            }
            expected_parent = Some(match expected_parent {
                Some(parent) => crate::concat_str!(parent, ".", segment),
                None => segment.to_string(),
            });
        }
    }
    false
}

fn check_is_extension_target(
    index: &WorkspaceIndex,
    location: &Location,
    decl_opt: Option<&crate::indexer::DeclarationItem>,
    query: &ReferenceQuery<'_>,
) -> bool {
    let check_tree = |root: Node<'_>, source: &str| -> Option<bool> {
        let leaf = find_node_at_position(root, location.range.start)?;
        let mut curr = leaf;
        while !matches!(curr.kind(), "function_declaration" | "property_declaration") {
            curr = curr.parent()?;
        }
        Some(crate::parser::extract_extension_receiver(&curr, source).is_some())
    };

    if location.uri == *query.current_url
        && let Some(root) = query.root
        && let Some(res) = check_tree(root, query.source)
    {
        return res;
    }

    if let Some(source) = index.get_file_source(&location.uri)
        && let Some(tree) = crate::parser::parse_source(&source)
        && let Some(res) = check_tree(tree.root_node(), &source)
    {
        return res;
    }

    decl_opt
        .and_then(|decl| decl.signature.as_deref())
        .is_some_and(|sig| sig.contains(&crate::concat_str!(".", query.symbol_name)))
}

pub fn resolve_reference_target(query: &ReferenceQuery<'_>) -> Option<ReferenceTarget> {
    let root = query.root?;
    let target_node = find_node_at_position(root, query.pos)?;

    if let Some(local_target) = resolve_local_target(query, target_node) {
        return Some(local_target);
    }

    if let Some(decl_target) = resolve_current_file_declaration_target(query) {
        return Some(decl_target);
    }

    resolve_definition_target(query, root)
}

fn resolve_current_file_declaration_target(query: &ReferenceQuery<'_>) -> Option<ReferenceTarget> {
    let file_symbols = query.index.files.get(query.current_url)?;
    let decl = file_symbols.declarations.iter().find(|decl_item| {
        decl_item.name == query.symbol_name
            && is_position_in_range(query.pos, decl_item.selection_range)
    })?;

    let target_location = Location {
        uri: query.current_url.clone(),
        range: decl.selection_range,
    };
    let is_extension = check_is_extension_target(query.index, &target_location, Some(decl), query);

    Some(ReferenceTarget {
        name: query.symbol_name.to_string(),
        location: target_location,
        is_local: false,
        local_scope_node_range: None,
        is_private: is_decl_or_container_private(file_symbols, decl),
        is_extension,
        is_override: decl.is_override,
        kind: Some(decl.kind),
        container_name: decl.container_name.clone(),
        signature: decl.signature.clone(),
    })
}

fn resolve_definition_target(
    query: &ReferenceQuery<'_>,
    root: Node<'_>,
) -> Option<ReferenceTarget> {
    let def_loc = find_definition_with_root(
        query.index,
        query.current_url,
        query.symbol_name,
        query.pos,
        query.source,
        Some(root),
    )?;

    let target_file_symbols = query.index.files.get(&def_loc.uri);
    let target_decl = target_file_symbols.and_then(|file_symbols| {
        file_symbols.declarations.iter().find(|decl| {
            decl.name == query.symbol_name
                && (decl.selection_range == def_loc.range
                    || is_position_in_range(def_loc.range.start, decl.range))
        })
    });

    let is_private = match (target_file_symbols, target_decl) {
        (Some(symbols), Some(decl)) => is_decl_or_container_private(symbols, decl),
        _ => false,
    };
    let is_extension = check_is_extension_target(query.index, &def_loc, target_decl, query);
    let is_override = target_decl.is_some_and(|decl| decl.is_override);

    Some(ReferenceTarget {
        name: query.symbol_name.to_string(),
        location: def_loc,
        is_local: false,
        local_scope_node_range: None,
        is_private,
        is_extension,
        is_override,
        kind: target_decl.map(|decl| decl.kind),
        container_name: target_decl.and_then(|decl| decl.container_name.clone()),
        signature: target_decl.and_then(|decl| decl.signature.clone()),
    })
}

pub fn is_declaration_name_node(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        "function_declaration"
        | "class_declaration"
        | "object_declaration"
        | "companion_object"
        | "type_alias"
        | "enum_entry"
        | "parameter"
        | "class_parameter"
        | "variable_declaration"
        | "catch_block" => {
            find_identifier_node(&parent).is_some_and(|ident| ident.id() == node.id())
        }
        "lambda_parameters" => true,
        _ => false,
    }
}
