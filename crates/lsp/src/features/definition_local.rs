use tower_lsp::lsp_types::{Location, Position, SymbolKind, Url};
use tree_sitter::Node;

use super::definition::DefinitionQuery;
use super::definition_match::{pick_best_decl_with_overload, pick_best_match_with_overload};
use crate::position::node_to_range;
use crate::syntax::scope::BindingRole;
use crate::syntax::symbol_item::{DeclarationItem, FileSymbols, find_identifier_node};

fn is_file_or_class_member_declaration(node: &Node) -> bool {
    if node.kind() == "class_parameter" && crate::parser::is_class_parameter_property(node) {
        return true;
    }
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

pub fn collect_indexed_containers_at_pos(
    file_syms: &FileSymbols,
    pos: Position,
    containers: &mut Vec<String>,
) {
    for d in &file_syms.declarations {
        if super::references::is_position_in_range(pos, d.range) {
            if let Some(ref c) = d.container_name {
                containers.push(c.to_string());
            }
            if matches!(
                d.kind,
                SymbolKind::CLASS | SymbolKind::INTERFACE | SymbolKind::OBJECT | SymbolKind::ENUM
            ) {
                let qualified = match d.container_name.as_deref() {
                    Some(c) => common::concat_str!(c, ".", d.name.as_str()),
                    None => d.name.to_string(),
                };
                containers.push(qualified);
            }
        }
    }
}

pub fn find_current_file_definition<F>(
    current_file: Option<&FileSymbols>,
    query: &DefinitionQuery<'_>,
    is_visible: &F,
) -> Option<Location>
where
    F: Fn(&DeclarationItem) -> bool,
{
    let file_symbols = current_file?;
    let matching = file_symbols
        .declarations
        .iter()
        .filter(|decl| decl.name == query.symbol_name && is_visible(decl));
    let decl = pick_best_decl_with_overload(
        matching,
        query.is_call,
        query.raw_args,
        query.inferred_types,
        query.index,
        Some(query.current_url),
        query.target_location,
    )?;
    if !decl.is_private
        && !decl.is_expect
        && let Some(decls) = query.index.name_to_decls.get(query.symbol_name)
        && let Some(expect_loc) = pick_best_match_with_overload(
            decls.iter().filter(|(url, cand)| {
                cand.is_expect
                    && !cand.is_private
                    && cand.container_name == decl.container_name
                    && is_visible(cand)
                    && query
                        .index
                        .files
                        .get(url.as_ref())
                        .and_then(|f| f.package_name.as_deref())
                        == file_symbols.package_name.as_deref()
            }),
            query.is_call,
            query.raw_args,
            query.inferred_types,
            query.index,
            query.target_location,
        )
    {
        return Some(expect_loc);
    }
    Some(Location::new(
        query.current_url.clone(),
        decl.selection_range,
    ))
}
