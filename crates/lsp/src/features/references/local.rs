use tower_lsp::lsp_types::{Location, SymbolKind};
use tree_sitter::Node;

use super::search::collect_matching_identifiers;
use super::target::{ReferenceTarget, is_declaration_name_node, is_position_in_range};
use super::{
    MAX_SLOWEST_OCCURRENCES, OccurrenceTiming, ReferenceQuery, merge_top_slowest,
    should_record_slowest,
};
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

pub fn find_local_references(
    query: &ReferenceQuery<'_>,
    target: &ReferenceTarget,
) -> (Vec<Location>, Vec<OccurrenceTiming>) {
    let Some(root) = query.root else {
        return (Vec::new(), Vec::new());
    };
    let Some(scope_range) = target.local_scope_node_range else {
        return (Vec::new(), Vec::new());
    };

    let mut candidate_nodes = Vec::new();
    collect_matching_identifiers(root, &target.name, query.source, &mut candidate_nodes);

    let mut results = Vec::new();
    let mut slowest = Vec::new();

    for cand_node in candidate_nodes {
        let cand_range = node_to_range(&cand_node);
        if !is_position_in_range(cand_range.start, scope_range) {
            continue;
        }

        let cand_start = if query.track_slowest {
            Some(std::time::Instant::now())
        } else {
            None
        };
        let before_count = results.len();

        if cand_range == target.location.range {
            if query.include_declaration {
                results.push(Location {
                    uri: query.current_url.clone(),
                    range: cand_range,
                });
            }
        } else if check_local_candidate(query, target, cand_node) {
            results.push(Location {
                uri: query.current_url.clone(),
                range: cand_range,
            });
        }

        if let Some(start) = cand_start {
            let elapsed = start.elapsed();
            if should_record_slowest(&slowest, elapsed) {
                let matched = results.len() > before_count;
                merge_top_slowest(
                    &mut slowest,
                    [OccurrenceTiming {
                        uri: query.current_url.clone(),
                        range: cand_range,
                        elapsed,
                        matched,
                    }],
                    MAX_SLOWEST_OCCURRENCES,
                );
            }
        }
    }

    (results, slowest)
}

fn check_local_candidate(
    query: &ReferenceQuery<'_>,
    target: &ReferenceTarget,
    cand_node: Node<'_>,
) -> bool {
    if is_declaration_name_node(&cand_node) {
        return false;
    }
    let is_callable_ref = is_callable_reference_target(&cand_node);
    let role = if is_callable_ref {
        BindingRole::CallableRef
    } else if is_call_callee(&cand_node, query.source) {
        BindingRole::DirectCall
    } else {
        BindingRole::Value
    };
    let Some(found_node) = find_local_binding_node(cand_node, &target.name, query.source, role)
    else {
        return false;
    };
    let id_range = find_identifier_node(&found_node)
        .map(|n| node_to_range(&n))
        .unwrap_or_else(|| node_to_range(&found_node));
    id_range == target.location.range
}
