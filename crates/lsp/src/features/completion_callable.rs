use std::collections::HashSet;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, InsertTextFormat, Position, SymbolKind, Url,
};
use tree_sitter::Node;

use super::auto_import::resolve_auto_import_candidate;
use super::completion_candidate::CompletionCandidateTracker;
use super::completion_item::{
    contains_ignore_ascii_case, decl_completion_key, declaration_to_callable_reference_item,
    starts_with_ignore_ascii_case,
};
use super::receiver_type::{collect_type_containers, resolve_receiver_type};
use crate::indexer::{DeclarationItem, WorkspaceIndex, is_source_url};
use crate::parser::find_node_at_position;
use crate::syntax::scope::{
    collect_enclosing_classes, collect_local_scope_symbols, find_enclosing_class_name,
};
use common::concat_str;

const MAX_COMPLETIONS: usize = 1000;

/// Find member or extension callables for `Receiver::` references
pub fn find_receiver_callable_completions(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root_node: Option<Node>,
    receiver: &str,
    partial: &str,
) -> Vec<CompletionItem> {
    let Some(type_name) =
        resolve_receiver_type(index, current_url, receiver, pos, source, root_node)
    else {
        return Vec::new();
    };

    let enclosing = root_node.and_then(|root| {
        find_node_at_position(root, pos).and_then(|node| find_enclosing_class_name(node, source))
    });
    let can_access_private = receiver == "this"
        || receiver.starts_with("this@")
        || enclosing
            .as_deref()
            .is_some_and(|enc| crate::syntax::hierarchy::containers_match(enc, &type_name));

    let partial_lower = partial.to_lowercase();
    let mut tracker = CompletionCandidateTracker::default();
    let mut seen_keys = HashSet::new();

    let mut add_candidate = |decl: &DeclarationItem, url: &Url| {
        if decl.is_private && !can_access_private {
            return;
        }
        if !matches!(
            decl.kind,
            SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::PROPERTY
        ) {
            return;
        }
        if contains_ignore_ascii_case(&decl.name, &partial_lower) {
            let is_src = is_source_url(url);
            tracker.record_callable_reference(decl, is_src, &mut seen_keys, MAX_COMPLETIONS);
        }
    };

    let containers = collect_type_containers(index, &type_name);
    for container in &containers {
        if let Some(decls) = index.container_to_decls.get(container.as_str()) {
            for (url, decl) in decls {
                add_candidate(decl, url);
            }
        }
    }

    if contains_ignore_ascii_case("class", &partial_lower) {
        tracker.items.push(CompletionItem {
            label: "class".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(concat_str!("KClass<", type_name, ">")),
            insert_text: Some("class".to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
        tracker.is_source.push(true);
    }

    tracker.sort_members();

    tracker.items
}

/// Find accessible callables in scope for unbound `::` references
pub fn find_unbound_callable_completions(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    partial: &str,
    root_node: Option<Node>,
    auto_import_enabled: bool,
) -> Vec<CompletionItem> {
    let partial_lower = partial.to_lowercase();
    let mut completions = Vec::new();
    let mut seen_labels = HashSet::new();

    collect_unbound_local_callables(
        root_node,
        source,
        pos,
        &partial_lower,
        &mut seen_labels,
        &mut completions,
    );

    let enclosing_classes = collect_enclosing_classes(root_node, pos, source);
    collect_unbound_file_callables(
        index,
        current_url,
        &partial_lower,
        &enclosing_classes,
        &mut seen_labels,
        &mut completions,
    );

    collect_unbound_index_callables(
        index,
        current_url,
        &partial_lower,
        &enclosing_classes,
        &mut seen_labels,
        auto_import_enabled,
        &mut completions,
    );

    completions
}

fn collect_unbound_local_callables(
    root_node: Option<Node>,
    source: &str,
    pos: Position,
    partial_lower: &str,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
) {
    let Some(root) = root_node else {
        return;
    };
    let before_byte = crate::position::position_to_byte_offset(pos, source);
    let mut current = find_node_at_position(root, pos);
    while let Some(parent) = current {
        if matches!(
            parent.kind(),
            "source_file" | "class_body" | "class_declaration" | "object_declaration"
        ) {
            break;
        }
        let mut symbols = Vec::new();
        let mut local_seen = HashSet::new();
        collect_local_scope_symbols(
            parent,
            source,
            partial_lower,
            before_byte,
            &mut local_seen,
            &mut symbols,
        );
        for sym in symbols {
            if sym.is_function && seen_labels.insert(sym.name.clone()) {
                completions.push(CompletionItem {
                    label: sym.name.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: sym.signature,
                    insert_text: Some(sym.name),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    ..Default::default()
                });
            }
        }
        current = parent.parent();
    }
}

fn collect_unbound_file_callables(
    index: &WorkspaceIndex,
    current_url: &Url,
    partial_lower: &str,
    enclosing_classes: &HashSet<String>,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
) {
    let Some(file) = index.files.get(current_url) else {
        return;
    };
    for decl in &file.declarations {
        let is_visible = decl
            .container_name
            .as_ref()
            .is_none_or(|container| enclosing_classes.contains(container.as_str()));
        let is_callable = matches!(
            decl.kind,
            SymbolKind::FUNCTION
                | SymbolKind::CONSTRUCTOR
                | SymbolKind::CLASS
                | SymbolKind::PROPERTY
                | SymbolKind::VARIABLE
        );
        let key_str = decl_completion_key(decl);
        if is_visible
            && is_callable
            && contains_ignore_ascii_case(&decl.name, partial_lower)
            && seen_labels.insert(key_str)
        {
            completions.push(declaration_to_callable_reference_item(decl));
        }
    }
}

fn collect_unbound_index_callables(
    index: &WorkspaceIndex,
    current_url: &Url,
    partial_lower: &str,
    enclosing_classes: &HashSet<String>,
    seen_labels: &mut HashSet<String>,
    auto_import_enabled: bool,
    completions: &mut Vec<CompletionItem>,
) {
    let mut prefix_tracker = CompletionCandidateTracker::default();
    let mut sub_tracker = CompletionCandidateTracker::default();
    let current_file = index.files.get(current_url);
    let current_pkg = current_file.and_then(|f| f.package_name.as_deref());

    for (url, decl) in &index.top_level_decls {
        if url.as_ref() == current_url {
            continue;
        }
        let is_visible = decl
            .container_name
            .as_ref()
            .is_none_or(|container| enclosing_classes.contains(container.as_str()));
        let is_callable = matches!(
            decl.kind,
            SymbolKind::FUNCTION
                | SymbolKind::CONSTRUCTOR
                | SymbolKind::CLASS
                | SymbolKind::PROPERTY
                | SymbolKind::VARIABLE
        );
        if !is_visible || !is_callable {
            continue;
        }

        let matches_prefix = starts_with_ignore_ascii_case(&decl.name, partial_lower);
        let matches_sub = !matches_prefix && contains_ignore_ascii_case(&decl.name, partial_lower);
        if !matches_prefix && !matches_sub {
            continue;
        }

        let is_source = is_source_url(url);
        let tracker = if matches_prefix {
            if !prefix_tracker.can_accept_or_replace(&decl.name, is_source, MAX_COMPLETIONS) {
                continue;
            }
            &mut prefix_tracker
        } else {
            if prefix_tracker.items.len() >= MAX_COMPLETIONS
                || !sub_tracker.can_accept_or_replace(&decl.name, is_source, MAX_COMPLETIONS)
            {
                continue;
            }
            &mut sub_tracker
        };

        let auto_import_pkg = if auto_import_enabled {
            resolve_auto_import_candidate(index, url.as_ref(), decl, current_pkg, current_file)
        } else {
            None
        };
        tracker.record_callable_ref_with_import(
            decl,
            is_source,
            seen_labels,
            MAX_COMPLETIONS,
            auto_import_pkg,
            Some(current_url),
        );
        if matches_prefix
            && (prefix_tracker.is_saturated(MAX_COMPLETIONS)
                || (partial_lower.is_empty() && prefix_tracker.items.len() >= MAX_COMPLETIONS))
        {
            break;
        }
    }

    prefix_tracker.drain_into(sub_tracker, completions, MAX_COMPLETIONS);
}
