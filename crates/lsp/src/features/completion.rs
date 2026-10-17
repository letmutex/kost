use std::collections::HashSet;
use tower_lsp::lsp_types::{CompletionItem, Position, Url};
use tree_sitter::Node;

use super::auto_import::resolve_auto_import_candidate;
use super::call_argument::{
    CallArgumentCompletionQuery, collect_call_argument_completions, find_enclosing_call_name,
};
use super::completion_context::{CompletionContext, determine_completion_context};
use super::completion_item::{
    CompletionCandidateTracker, contains_ignore_ascii_case, decl_completion_key,
    starts_with_ignore_ascii_case,
};
use super::completion_member::find_member_completions;
use super::completion_scope::collect_local_scope_completions;
use super::lex_context::is_inside_comment_or_string;
use crate::indexer::{WorkspaceIndex, is_source_url};
use crate::parser::{find_node_at_position, parse_source};
use crate::position::position_to_byte_offset;
use crate::syntax::scope::collect_enclosing_classes;

const MAX_COMPLETIONS: usize = 1000;
pub use super::completion_item::{declaration_to_completion_item, map_symbol_kind};
pub use super::receiver_type::resolve_receiver_type;

/// Find completions at given cursor position
pub fn find_completions(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
) -> Vec<CompletionItem> {
    find_completions_with_config(index, current_url, pos, source, None, true)
}

/// Find completions at given cursor position with optional pre-parsed root node
pub fn find_completions_with_root(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root: Option<Node>,
) -> Vec<CompletionItem> {
    find_completions_with_config(index, current_url, pos, source, root, true)
}

/// Find completions with configuration options
pub fn find_completions_with_config(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root: Option<Node>,
    auto_import_enabled: bool,
) -> Vec<CompletionItem> {
    let tree = root.is_none().then(|| parse_source(source)).flatten();
    let root_node = root.or_else(|| tree.as_ref().map(|t| t.root_node()));

    if is_inside_comment_or_string(root_node, pos, source) {
        return Vec::new();
    }

    match determine_completion_context(pos, source) {
        CompletionContext::MemberAccess { receiver, partial } => find_member_completions(
            index,
            current_url,
            pos,
            source,
            root_node,
            &receiver,
            &partial,
        ),
        CompletionContext::CallableReference {
            receiver: Some(receiver),
            partial,
        } => super::completion_callable::find_receiver_callable_completions(
            index,
            current_url,
            pos,
            source,
            root_node,
            &receiver,
            &partial,
        ),
        CompletionContext::CallableReference {
            receiver: None,
            partial,
        } => super::completion_callable::find_unbound_callable_completions(
            index,
            current_url,
            pos,
            source,
            &partial,
            root_node,
            auto_import_enabled,
        ),
        CompletionContext::Scope { partial } => {
            if partial.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Vec::new();
            }
            find_scope_completions(
                index,
                current_url,
                pos,
                source,
                &partial,
                root_node,
                auto_import_enabled,
            )
        }
    }
}

fn find_scope_completions(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    partial: &str,
    root: Option<Node>,
    auto_import_enabled: bool,
) -> Vec<CompletionItem> {
    let partial_lower = partial.to_lowercase();
    let mut completions = Vec::new();
    let mut seen_labels = HashSet::new();

    if let Some((receiver, callee)) = find_enclosing_call_name(pos, source) {
        collect_call_argument_completions(
            CallArgumentCompletionQuery {
                index,
                current_url,
                receiver: receiver.as_deref(),
                callee: &callee,
                partial_lower: &partial_lower,
                pos,
                source,
                root,
            },
            &mut seen_labels,
            &mut completions,
        );
    }

    collect_enclosing_scope_symbols(
        root,
        pos,
        source,
        &partial_lower,
        &mut seen_labels,
        &mut completions,
    );

    let enclosing_classes = collect_enclosing_classes(root, pos, source);
    collect_index_scope_declarations(
        index,
        current_url,
        &partial_lower,
        &enclosing_classes,
        &mut seen_labels,
        &mut completions,
        auto_import_enabled,
    );

    completions
}

fn collect_file_scope_declarations(
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
            .is_none_or(|c| enclosing_classes.contains(c.as_str()));
        let key_str = decl_completion_key(decl);
        if is_visible
            && contains_ignore_ascii_case(&decl.name, partial_lower)
            && seen_labels.insert(key_str)
        {
            completions.push(declaration_to_completion_item(decl));
        }
    }
}

fn collect_top_level_scope_declarations(
    index: &WorkspaceIndex,
    current_url: &Url,
    partial_lower: &str,
    enclosing_classes: &HashSet<String>,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
    auto_import_enabled: bool,
) {
    let mut prefix_tracker = CompletionCandidateTracker::default();
    let mut sub_tracker = CompletionCandidateTracker::default();

    let current_file = index.files.get(current_url);
    let current_pkg = current_file.and_then(|file_symbols| file_symbols.package_name.as_deref());

    for (url, decl) in &index.top_level_decls {
        if url.as_ref() == current_url {
            continue;
        }
        let is_visible = decl
            .container_name
            .as_ref()
            .is_none_or(|container| enclosing_classes.contains(container.as_str()));
        if !is_visible {
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
        tracker.record_with_import(
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

fn collect_index_scope_declarations(
    index: &WorkspaceIndex,
    current_url: &Url,
    partial_lower: &str,
    enclosing_classes: &HashSet<String>,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
    auto_import_enabled: bool,
) {
    collect_file_scope_declarations(
        index,
        current_url,
        partial_lower,
        enclosing_classes,
        seen_labels,
        completions,
    );
    collect_top_level_scope_declarations(
        index,
        current_url,
        partial_lower,
        enclosing_classes,
        seen_labels,
        completions,
        auto_import_enabled,
    );
}

fn collect_enclosing_scope_symbols(
    root: Option<Node>,
    pos: Position,
    source: &str,
    partial_lower: &str,
    seen_labels: &mut HashSet<String>,
    completions: &mut Vec<CompletionItem>,
) {
    let cursor_byte = position_to_byte_offset(pos, source);
    let mut current = root.and_then(|r| find_node_at_position(r, pos));
    while let Some(parent) = current {
        if matches!(
            parent.kind(),
            "source_file" | "class_body" | "class_declaration" | "object_declaration"
        ) {
            break;
        }
        collect_local_scope_completions(
            parent,
            source,
            partial_lower,
            cursor_byte,
            seen_labels,
            completions,
        );
        current = parent.parent();
    }
}
