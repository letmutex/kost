use tower_lsp::lsp_types::{Location, SymbolKind};
use tree_sitter::Node;

use super::ReferenceQuery;
use super::target::{ReferenceTarget, is_position_in_range};
use crate::features::definition::{BindingRole, find_local_binding_node};
use crate::position::node_to_range;
use crate::syntax::ast_util::{is_call_callee, is_callable_reference_target};
use crate::syntax::symbol_item::find_identifier_node;

pub fn resolve_local_target(
    query: &ReferenceQuery<'_>,
    target_node: Node<'_>,
) -> Option<ReferenceTarget> {
    let callable = find_enclosing_callable(target_node)?;
    let callable_range = node_to_range(&callable);

    if is_local_declaration_name_node(target_node, callable) {
        return Some(ReferenceTarget {
            name: query.symbol_name.to_string(),
            location: Location {
                uri: query.current_url.clone(),
                range: node_to_range(&target_node),
            },
            is_local: true,
            local_scope_node_range: Some(callable_range),
            is_private: true,
            is_extension: false,
            is_override: false,
            kind: Some(SymbolKind::VARIABLE),
            container_name: None,
            signature: None,
        });
    }

    let is_callable_ref = is_callable_reference_target(&target_node);
    let role = if is_callable_ref {
        BindingRole::CallableRef
    } else if is_call_callee(&target_node, query.source) {
        BindingRole::DirectCall
    } else {
        BindingRole::Value
    };
    let found_node = find_local_binding_node(target_node, query.symbol_name, query.source, role)?;

    let id_range = find_identifier_node(&found_node)
        .map(|n| node_to_range(&n))
        .unwrap_or_else(|| node_to_range(&found_node));

    if is_position_in_range(id_range.start, callable_range) {
        let kind = if found_node.kind() == "function_declaration" {
            SymbolKind::FUNCTION
        } else {
            SymbolKind::VARIABLE
        };
        return Some(ReferenceTarget {
            name: query.symbol_name.to_string(),
            location: Location {
                uri: query.current_url.clone(),
                range: id_range,
            },
            is_local: true,
            local_scope_node_range: Some(callable_range),
            is_private: true,
            is_extension: false,
            is_override: false,
            kind: Some(kind),
            container_name: None,
            signature: None,
        });
    }

    None
}

fn find_enclosing_callable(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = if let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "function_declaration"
                | "property_declaration"
                | "variable_declaration"
                | "class_declaration"
                | "object_declaration"
        ) && find_identifier_node(&parent).is_some_and(|id| id.id() == node.id())
        {
            parent.parent()
        } else {
            Some(parent)
        }
    } else {
        None
    };

    while let Some(parent) = current {
        let kind = parent.kind();
        if matches!(
            kind,
            "function_declaration"
                | "anonymous_function"
                | "lambda_literal"
                | "secondary_constructor"
                | "init_block"
                | "catch_block"
                | "getter"
                | "setter"
        ) {
            return Some(parent);
        }
        if matches!(
            kind,
            "class_declaration" | "object_declaration" | "companion_object" | "source_file"
        ) {
            break;
        }
        current = parent.parent();
    }
    None
}

fn is_local_declaration_name_node(node: Node<'_>, callable: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        "parameter" => find_identifier_node(&parent).is_some_and(|ident| ident.id() == node.id()),
        "variable_declaration" | "function_declaration" => {
            callable.start_byte() <= parent.start_byte()
                && parent.end_byte() <= callable.end_byte()
                && find_identifier_node(&parent).is_some_and(|ident| ident.id() == node.id())
        }
        "catch_block" => find_identifier_node(&parent).is_some_and(|ident| ident.id() == node.id()),
        "lambda_parameters" => true,
        _ => false,
    }
}
