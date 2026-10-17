use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tower_lsp::lsp_types::{Location, Url};
use tree_sitter::Node;

use super::ReferenceQuery;
use super::filter::{find_producer_names, is_file_candidate_with_producers};
use super::hierarchy::check_candidate_node;
use super::target::{ReferenceTarget, is_declaration_name_node, is_position_in_range};
use crate::features::definition::{BindingRole, find_local_binding_node};
use crate::indexer::{WorkspaceIndex, is_source_url};
use crate::parser::{extract_identifier_text, parse_source};
use crate::position::node_to_range;
use crate::syntax::ast_util::{is_call_callee, is_callable_reference_target};
use crate::syntax::symbol_item::find_identifier_node;

pub fn find_local_references(
    query: &ReferenceQuery<'_>,
    target: &ReferenceTarget,
) -> Vec<Location> {
    let Some(root) = query.root else {
        return Vec::new();
    };
    let Some(scope_range) = target.local_scope_node_range else {
        return Vec::new();
    };

    let mut candidate_nodes = Vec::new();
    collect_matching_identifiers(root, &target.name, query.source, &mut candidate_nodes);

    let mut results = Vec::new();
    for cand_node in candidate_nodes {
        let cand_range = node_to_range(&cand_node);
        if !is_position_in_range(cand_range.start, scope_range) {
            continue;
        }

        if cand_range == target.location.range {
            if query.include_declaration {
                results.push(Location {
                    uri: query.current_url.clone(),
                    range: cand_range,
                });
            }
            continue;
        }

        if is_declaration_name_node(&cand_node) {
            continue;
        }

        let is_callable_ref = is_callable_reference_target(&cand_node);
        let role = if is_callable_ref {
            BindingRole::CallableRef
        } else if is_call_callee(&cand_node, query.source) {
            BindingRole::DirectCall
        } else {
            BindingRole::Value
        };
        if let Some(found_node) =
            find_local_binding_node(cand_node, &target.name, query.source, role)
        {
            let id_range = find_identifier_node(&found_node)
                .map(|n| node_to_range(&n))
                .unwrap_or_else(|| node_to_range(&found_node));
            if id_range == target.location.range {
                results.push(Location {
                    uri: query.current_url.clone(),
                    range: cand_range,
                });
            }
        }
    }
    results
}

fn resolve_target_scope<'a>(
    index: &'a WorkspaceIndex,
    target: &ReferenceTarget,
) -> (Option<&'a str>, bool) {
    let Some(file_symbols) = index.files.get(&target.location.uri) else {
        return (None, false);
    };
    let pkg = file_symbols.package_name.as_deref();
    let decl = file_symbols.declarations.iter().find(|d| {
        d.selection_range == target.location.range
            || (d.name == target.name && is_position_in_range(target.location.range.start, d.range))
    });
    (pkg, decl.is_some_and(|d| d.container_name.is_none()))
}

fn find_private_references(query: &ReferenceQuery<'_>, target: &ReferenceTarget) -> Vec<Location> {
    if target.location.uri == *query.current_url {
        return scan_file_for_references(
            query.index,
            query.include_declaration,
            target,
            query.current_url,
            query.source,
            query.root,
        );
    }
    let Some(source) = query.index.get_file_source(&target.location.uri) else {
        return Vec::new();
    };
    scan_file_for_references(
        query.index,
        query.include_declaration,
        target,
        &target.location.uri,
        &source,
        None,
    )
}

fn resolve_target_producer_names(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
) -> Vec<smol_str::SmolStr> {
    let clean_container = target
        .container_name
        .as_deref()
        .map(|container| container.strip_suffix(".Companion").unwrap_or(container));
    let simple_container = clean_container.map(|clean| clean.rsplit('.').next().unwrap_or(clean));
    simple_container
        .map(|simple| find_producer_names(index, simple))
        .unwrap_or_default()
}

fn scan_remote_files(
    query: &ReferenceQuery<'_>,
    target: &ReferenceTarget,
    target_pkg: Option<&str>,
    is_top_level: bool,
    other_urls: Vec<&Url>,
    producer_names: &[smol_str::SmolStr],
) -> Vec<Location> {
    other_urls
        .into_par_iter()
        .flat_map(|candidate_url| {
            let disk_source = query.index.get_file_source(candidate_url);
            let Some(ref source) = disk_source else {
                return Vec::new();
            };
            if !is_file_candidate_with_producers(
                query.index,
                candidate_url,
                target,
                target_pkg,
                is_top_level,
                source,
                producer_names,
            ) {
                return Vec::new();
            }
            scan_file_for_references(
                query.index,
                query.include_declaration,
                target,
                candidate_url,
                source,
                None,
            )
        })
        .collect()
}

pub fn find_workspace_references(
    query: &ReferenceQuery<'_>,
    target: &ReferenceTarget,
) -> Vec<Location> {
    if target.is_private {
        return find_private_references(query, target);
    }

    let (target_pkg, is_top_level) = resolve_target_scope(query.index, target);
    let producer_names = resolve_target_producer_names(query.index, target);

    let other_urls: Vec<&Url> = query
        .index
        .files
        .keys()
        .filter(|candidate_url| {
            candidate_url.scheme() == "file"
                && is_source_url(candidate_url)
                && *candidate_url != query.current_url
        })
        .collect();

    let mut results = if is_file_candidate_with_producers(
        query.index,
        query.current_url,
        target,
        target_pkg,
        is_top_level,
        query.source,
        &producer_names,
    ) {
        scan_file_for_references(
            query.index,
            query.include_declaration,
            target,
            query.current_url,
            query.source,
            query.root,
        )
    } else {
        Vec::new()
    };

    let remote_results = scan_remote_files(
        query,
        target,
        target_pkg,
        is_top_level,
        other_urls,
        &producer_names,
    );

    results.extend(remote_results);
    results
}

fn scan_file_for_references(
    index: &WorkspaceIndex,
    include_declaration: bool,
    target: &ReferenceTarget,
    cand_url: &Url,
    source: &str,
    file_root_opt: Option<Node<'_>>,
) -> Vec<Location> {
    let parsed_tree;
    let file_root = if let Some(root) = file_root_opt {
        root
    } else {
        let Some(tree) = parse_source(source) else {
            return Vec::new();
        };
        parsed_tree = tree;
        parsed_tree.root_node()
    };

    let mut candidate_nodes = Vec::new();
    collect_matching_identifiers(file_root, &target.name, source, &mut candidate_nodes);

    let mut results = Vec::new();
    for &cand_node in &candidate_nodes {
        check_candidate_node(
            index,
            include_declaration,
            target,
            cand_url,
            cand_node,
            source,
            file_root,
            &mut results,
        );
    }
    results
}

fn collect_matching_identifiers<'a>(
    root: Node<'a>,
    target_name: &str,
    source: &'a str,
    candidates: &mut Vec<Node<'a>>,
) {
    let mut cursor = root.walk();
    let mut visited_children = false;
    loop {
        let node = cursor.node();
        let kind = node.kind();
        if !visited_children {
            if matches!(
                kind,
                "comment" | "line_comment" | "multiline_comment" | "package_header"
            ) {
                if cursor.goto_next_sibling() {
                    continue;
                } else if cursor.goto_parent() {
                    visited_children = true;
                    continue;
                } else {
                    break;
                }
            }
            if matches!(
                kind,
                "simple_identifier"
                    | "type_identifier"
                    | "identifier"
                    | "operator_identifier"
                    | "interpolated_identifier"
            ) && extract_identifier_text(&node, source) == Some(target_name)
            {
                candidates.push(node);
            }
            if cursor.goto_first_child() {
                continue;
            }
        }
        if cursor.goto_next_sibling() {
            visited_children = false;
        } else if cursor.goto_parent() {
            visited_children = true;
        } else {
            break;
        }
    }
}

pub fn deduplicate_locations(mut locations: Vec<Location>) -> Vec<Location> {
    locations.sort_by(|a, b| {
        a.uri
            .as_str()
            .cmp(b.uri.as_str())
            .then_with(|| a.range.start.line.cmp(&b.range.start.line))
            .then_with(|| a.range.start.character.cmp(&b.range.start.character))
            .then_with(|| a.range.end.line.cmp(&b.range.end.line))
            .then_with(|| a.range.end.character.cmp(&b.range.end.character))
    });
    locations.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);
    locations
}
