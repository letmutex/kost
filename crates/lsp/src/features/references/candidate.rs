use tower_lsp::lsp_types::{SymbolKind, Url};
use tree_sitter::Node;

use super::target::ReferenceTarget;
use crate::indexer::WorkspaceIndex;
use crate::syntax::ast_call_view::AstCallView;
use crate::syntax::ast_util::{is_call_callee, is_callable_reference_target};

pub fn is_callable_candidate(node: &Node, source: &str) -> bool {
    if is_call_callee(node, source) || is_callable_reference_target(node) {
        return true;
    }
    node.parent()
        .and_then(find_enclosing_import_header)
        .is_some()
}

pub fn find_enclosing_import_header(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if node.kind() == "import_header" {
            return Some(node);
        }
        if matches!(
            node.kind(),
            "source_file" | "class_body" | "function_body" | "block"
        ) {
            return None;
        }
        node = node.parent()?;
    }
}

pub(super) fn has_package_prefix(name: &str, pkg: &str) -> bool {
    name == pkg
        || name
            .strip_prefix(pkg)
            .is_some_and(|rest| rest.starts_with('.'))
}

pub fn is_import_header_matching_target(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    header: Node<'_>,
    source: &str,
) -> bool {
    let Some(imp) = crate::syntax::symbols::parse_import_header(header, source) else {
        return false;
    };
    if imp.is_star {
        return false;
    }
    let target_file = index.files.get(&target.location.uri);
    let target_pkg = target_file
        .and_then(|file| file.package_name.as_deref())
        .unwrap_or("");
    let expected_pkg = if let Some(ref target_container) = target.container_name {
        let clean = target_container
            .strip_suffix(".Companion")
            .unwrap_or(target_container);
        if clean.is_empty() {
            target_pkg.to_string()
        } else if !target_pkg.is_empty() && !has_package_prefix(clean, target_pkg) {
            crate::concat_str!(target_pkg, ".", clean)
        } else {
            clean.to_string()
        }
    } else {
        target_pkg.to_string()
    };

    super::filter::import_matches_target(&imp, target.name.as_str(), &expected_pkg)
}

pub fn is_arity_incompatible(target: &ReferenceTarget, cand_node: &Node<'_>) -> bool {
    if !matches!(
        target.kind,
        Some(SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR)
    ) {
        return false;
    }

    let Some(ref target_sig) = target.signature else {
        return false;
    };
    let Some((open_paren_idx, close_paren_idx)) =
        crate::syntax::signature::params::find_parameter_list_bounds(target_sig)
    else {
        return false;
    };

    let Some(arg_count) = count_call_site_arguments(cand_node) else {
        return false;
    };

    let sig_params = crate::syntax::signature::params::parse_signature_params(target_sig);
    if sig_params.is_empty()
        && target_sig[open_paren_idx + 1..close_paren_idx]
            .contains(|character: char| character.is_alphanumeric())
    {
        return false;
    }

    let min_params = sig_params
        .iter()
        .filter(|param| !param.has_default && !param.is_vararg)
        .count();
    let max_params = if sig_params.iter().any(|param| param.is_vararg) {
        usize::MAX
    } else {
        sig_params.len()
    };
    arg_count < min_params || arg_count > max_params
}

pub fn count_call_site_arguments(node: &Node) -> Option<usize> {
    let call_node = get_call_expression_for_callee(node)?;
    let view = AstCallView::from_node(call_node)?;
    Some(view.total_argument_count())
}

pub fn get_call_expression_for_callee<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    let parent = node.parent()?;
    let parent_kind = parent.kind();
    if parent_kind == "call_expression"
        && parent.child(0).is_some_and(|child| child.id() == node.id())
    {
        return Some(parent);
    }
    if parent_kind == "navigation_suffix" {
        let nav_expr = parent.parent()?;
        if nav_expr.kind() == "navigation_expression" {
            let call_expr = nav_expr.parent()?;
            if call_expr.kind() == "call_expression"
                && call_expr
                    .child(0)
                    .is_some_and(|child| child.id() == nav_expr.id())
            {
                return Some(call_expr);
            }
        }
    }
    if parent_kind == "navigation_expression" {
        let is_receiver = parent.child(0).is_some_and(|child| child.id() == node.id());
        if !is_receiver
            && let Some(call_expr) = parent.parent()
            && call_expr.kind() == "call_expression"
            && call_expr
                .child(0)
                .is_some_and(|child| child.id() == parent.id())
        {
            return Some(call_expr);
        }
    }
    None
}

pub(super) fn iter_overload_declarations<'a>(
    index: &'a WorkspaceIndex,
    target: &'a ReferenceTarget,
    cand_url: &'a Url,
) -> impl Iterator<Item = &'a crate::indexer::DeclarationItem> {
    let container = target.container_name.as_deref();
    let from_container = container
        .and_then(|c| index.container_to_decls.get(c))
        .map(|decls| decls.iter().map(|(u, d)| (u, d.as_ref())));
    let from_names = if container.is_none() {
        index
            .name_to_decls
            .get(target.name.as_str())
            .map(|decls| decls.iter().map(|(u, d)| (u, d.as_ref())))
    } else {
        None
    };

    let target_file = index.files.get(&target.location.uri);
    let target_pkg = target_file
        .and_then(|f| f.package_name.as_deref())
        .unwrap_or("");
    let target_full = container.map(|c| {
        if target_pkg.is_empty() || has_package_prefix(c, target_pkg) {
            c.to_string()
        } else {
            crate::concat_str!(target_pkg, ".", c)
        }
    });

    let cand_file = index.files.get(cand_url);
    let cand_pkg = cand_file
        .and_then(|f| f.package_name.as_deref())
        .unwrap_or("");

    from_container
        .into_iter()
        .flatten()
        .chain(from_names.into_iter().flatten())
        .filter(move |(url, decl)| {
            if decl.name != target.name {
                return false;
            }
            if url.as_ref() == &target.location.uri && decl.selection_range == target.location.range
            {
                return false;
            }
            if let Some(ref tf) = target_full {
                let Some(ref dc) = decl.container_name else {
                    return false;
                };
                let decl_file = index.files.get(url.as_ref());
                let decl_pkg = decl_file
                    .and_then(|f| f.package_name.as_deref())
                    .unwrap_or("");
                let decl_full = if decl_pkg.is_empty() || has_package_prefix(dc, decl_pkg) {
                    dc.to_string()
                } else {
                    crate::concat_str!(decl_pkg, ".", dc)
                };
                super::hierarchy::is_in_container_hierarchy(index, &decl_full, tf)
                    || super::hierarchy::is_in_container_hierarchy(index, tf, &decl_full)
            } else {
                if decl.container_name.is_some() {
                    return false;
                }
                let decl_file = index.files.get(url.as_ref());
                let decl_pkg = decl_file
                    .and_then(|f| f.package_name.as_deref())
                    .unwrap_or("");
                url.as_ref() == cand_url
                    || decl_pkg == cand_pkg
                    || cand_file.is_some_and(|f| {
                        f.imports.iter().any(|imp| {
                            super::filter::import_matches_target(imp, &target.name, decl_pkg)
                        })
                    })
            }
        })
        .map(|(_, decl)| decl)
}
