use tower_lsp::lsp_types::{CompletionItem, Position, Url};
use tree_sitter::Node;

use super::completion_item::{CompletionCandidateTracker, contains_ignore_ascii_case};
use super::receiver_type::{collect_type_containers, resolve_receiver_type};
use crate::concat_str;
use crate::indexer::{DeclarationItem, WorkspaceIndex, is_source_url};
use crate::parser::find_node_at_position;
use crate::syntax::scope::find_enclosing_class_name;

pub fn find_member_completions(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root_node: Option<Node>,
    receiver: &str,
    partial: &str,
) -> Vec<CompletionItem> {
    let parsed_tree = root_node
        .is_none()
        .then(|| crate::parser::parse_source(source))
        .flatten();
    let effective_root = root_node.or_else(|| parsed_tree.as_ref().map(|t| t.root_node()));

    if is_numeric_receiver(effective_root, pos, source, receiver) {
        return Vec::new();
    }

    let Some(type_name) =
        resolve_receiver_type(index, current_url, receiver, pos, source, effective_root)
    else {
        return find_package_completions(index, receiver, partial);
    };

    let enclosing = effective_root.and_then(|r| {
        find_node_at_position(r, pos).and_then(|n| find_enclosing_class_name(n, source))
    });
    let can_access_private = receiver == "this"
        || receiver.starts_with("this@")
        || enclosing.as_deref() == Some(&type_name);

    let partial_lower = partial.to_lowercase();
    let mut tracker = CompletionCandidateTracker::default();
    let mut seen_keys = std::collections::HashSet::new();

    let mut add_candidate = |decl: &DeclarationItem, url: &Url| {
        if decl.is_private && !can_access_private {
            return;
        }
        if contains_ignore_ascii_case(&decl.name, &partial_lower) {
            let is_src = is_source_url(url);
            tracker.record(decl, is_src, &mut seen_keys, 1000);
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

    tracker.sort_members();
    tracker.items
}

fn find_package_completions(
    index: &WorkspaceIndex,
    receiver: &str,
    partial: &str,
) -> Vec<CompletionItem> {
    let partial_lower = partial.to_lowercase();
    let mut tracker = CompletionCandidateTracker::default();
    let mut seen_keys = std::collections::HashSet::new();
    let prefix = concat_str!(receiver, ".");

    for (url, file) in &index.files {
        if file.package_name.as_deref() == Some(receiver) {
            for decl in &file.declarations {
                if decl.container_name.is_none()
                    && !decl.is_private
                    && contains_ignore_ascii_case(&decl.name, &partial_lower)
                {
                    let is_src = is_source_url(url);
                    tracker.record(decl, is_src, &mut seen_keys, 1000);
                }
            }
        } else if let Some(pkg) = &file.package_name
            && let Some(rest) = pkg.strip_prefix(&prefix)
        {
            let subpkg = rest.split('.').next().unwrap_or(rest);
            if contains_ignore_ascii_case(subpkg, &partial_lower) && !seen_keys.contains(subpkg) {
                seen_keys.insert(subpkg.to_string());
                tracker.push_module(subpkg.to_string());
            }
        }
    }

    tracker.sort_by_label();
    tracker.items
}

fn is_numeric_literal_node(node: Node) -> bool {
    matches!(
        node.kind(),
        "integer_literal"
            | "real_literal"
            | "float_literal"
            | "hex_literal"
            | "bin_literal"
            | "long_literal"
            | "unsigned_literal"
    )
}

fn is_numeric_receiver(
    root_node: Option<Node>,
    pos: Position,
    source: &str,
    receiver: &str,
) -> bool {
    if let Some(root) = root_node {
        let offset = crate::position::position_to_byte_offset(pos, source);
        let prefix = &source[..offset];
        if let Some(dot_idx) = prefix.rfind('.') {
            let before_dot = source[..dot_idx].trim_end();
            if !before_dot.is_empty() {
                let target_byte = before_dot.len() - 1;
                if let Some(node) = root.descendant_for_byte_range(target_byte, before_dot.len())
                    && (is_numeric_literal_node(node)
                        || node.parent().is_some_and(is_numeric_literal_node))
                {
                    return true;
                }
            }
        }
    }
    receiver
        .trim_start()
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
}
