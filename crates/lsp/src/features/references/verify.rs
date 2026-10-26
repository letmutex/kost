use tower_lsp::lsp_types::{SymbolKind, Url};
use tree_sitter::Node;

use super::candidate::{
    find_enclosing_import_header, get_call_expression_for_callee, is_arity_incompatible,
    is_callable_candidate, is_import_header_matching_target,
};
use super::hierarchy::conforms_to_container_or_companion;
use super::overload::verify_call_overload_signature;
use super::target::{ReferenceTarget, is_declaration_name_node, is_position_in_range};
use crate::indexer::WorkspaceIndex;

fn is_callable_target(target: &ReferenceTarget) -> bool {
    if matches!(
        target.kind,
        Some(
            SymbolKind::FUNCTION
                | SymbolKind::METHOD
                | SymbolKind::CONSTRUCTOR
                | SymbolKind::CLASS
                | SymbolKind::INTERFACE
                | SymbolKind::OBJECT
        )
    ) {
        return true;
    }
    target.signature.as_deref().is_some_and(|sig| {
        crate::inference::signature::SignatureInfo::parse(sig)
            .and_then(|info| info.return_type)
            .is_some_and(|ret| ret.is_function())
    })
}

pub fn verify_candidate_target(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    cand_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
    file_root: Option<Node<'_>>,
) -> bool {
    if is_declaration_name_node(cand_node) {
        return false;
    }
    if let Some(import_header) = find_enclosing_import_header(*cand_node) {
        return is_import_header_matching_target(index, target, import_header, source);
    }
    if get_call_expression_for_callee(cand_node).is_some() && !is_callable_target(target) {
        return false;
    }
    if matches!(
        target.kind,
        Some(SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR)
    ) && !is_callable_candidate(cand_node, source)
    {
        return false;
    }
    if is_arity_incompatible(target, cand_node) {
        return false;
    }

    if let Some(ref target_container) = target.container_name {
        verify_member_target(
            index,
            target,
            target_container.as_str(),
            cand_node,
            cand_url,
            source,
            file_root,
        )
    } else {
        verify_toplevel_target(index, target, cand_node, cand_url, source, file_root)
    }
}

fn verify_member_target(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    target_container: &str,
    cand_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
    file_root: Option<Node<'_>>,
) -> bool {
    let receiver_node_opt = crate::syntax::ast_util::extract_receiver_node(cand_node);
    if let Some(ref receiver_node) = receiver_node_opt
        && super::receiver_filter::is_receiver_incompatible(index, target, receiver_node, source)
    {
        return false;
    }

    let target_file = index.files.get(&target.location.uri);
    let target_pkg = target_file.and_then(|file| file.package_name.as_deref());
    let full_target_container = if let Some(pkg) = target_pkg
        && !pkg.is_empty()
        && !super::candidate::has_package_prefix(target_container, pkg)
    {
        common::concat_str!(pkg, ".", target_container)
    } else {
        target_container.to_string()
    };

    let receiver_conforms = if let Some(receiver_node) = receiver_node_opt {
        verify_explicit_receiver_conformance(
            index,
            &full_target_container,
            &receiver_node,
            cand_url,
            source,
            file_root,
        )
    } else {
        if !matches_lexical_binding(target, cand_node, cand_url, source) {
            return false;
        }
        verify_implicit_receiver_conformance(
            index,
            &full_target_container,
            cand_node,
            cand_url,
            source,
            file_root,
        )
    };

    if !receiver_conforms {
        return false;
    }

    verify_call_overload_signature(index, target, cand_node, cand_url, source, file_root)
}

fn verify_explicit_receiver_conformance(
    index: &WorkspaceIndex,
    target_container: &str,
    receiver_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
    file_root: Option<Node<'_>>,
) -> bool {
    let rec_kind = receiver_node.kind();
    if receiver_node
        .parent()
        .is_some_and(|parent| parent.kind() == "callable_reference")
        && let Some(rec_ident) = source.get(receiver_node.byte_range()).map(str::trim)
    {
        let full_rec = crate::inference::resolve_full_type_name(index, cand_url, rec_ident);
        if conforms_to_container_or_companion(index, &full_rec, target_container) {
            return true;
        }
    }
    if let Some(inferred_type) =
        crate::inference::infer_node_type(index, cand_url, *receiver_node, source, file_root)
    {
        let full_type =
            crate::inference::resolve_full_type_name(index, cand_url, inferred_type.base_name());
        return conforms_to_container_or_companion(index, &full_type, target_container);
    }
    if rec_kind == "simple_identifier"
        && let Some(explicit_type) =
            super::receiver_filter::find_local_receiver_explicit_type(receiver_node, source)
    {
        let full_type = crate::inference::resolve_full_type_name(index, cand_url, &explicit_type);
        return conforms_to_container_or_companion(index, &full_type, target_container);
    }
    false
}

fn verify_implicit_receiver_conformance(
    index: &WorkspaceIndex,
    target_container: &str,
    cand_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
    file_root: Option<Node<'_>>,
) -> bool {
    let cand_file = index.files.get(cand_url);
    let cand_pkg = cand_file.and_then(|file| file.package_name.as_deref());
    let pos = crate::position::point_to_position(cand_node.start_position());

    let enclosing =
        crate::syntax::scope_enclosing::collect_enclosing_classes(file_root, pos, source);
    for cls in &enclosing {
        let full = if let Some(pkg) = cand_pkg.filter(|p| !p.is_empty()) {
            common::concat_str!(pkg, ".", cls)
        } else {
            cls.clone()
        };
        if conforms_to_container_or_companion(index, &full, target_container) {
            return true;
        }
    }

    if let Some(inferred_this) =
        crate::inference::infer_receiver_type(index, cand_url, "this", pos, source, file_root)
    {
        let full_this =
            crate::inference::resolve_full_type_name(index, cand_url, inferred_this.base_name());
        return conforms_to_container_or_companion(index, &full_this, target_container);
    }

    false
}

fn matches_lexical_binding(
    target: &ReferenceTarget,
    cand_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
) -> bool {
    let role = if crate::syntax::ast_util::is_callable_reference_target(cand_node) {
        crate::syntax::scope::BindingRole::CallableRef
    } else if matches!(
        target.kind,
        Some(SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR)
    ) {
        crate::syntax::scope::BindingRole::DirectCall
    } else {
        crate::syntax::scope::BindingRole::Value
    };
    if let Some(binding) =
        crate::syntax::scope::find_lexical_binding_node(*cand_node, &target.name, source, role)
    {
        if cand_url != &target.location.uri {
            return false;
        }
        if let Some(parent) = binding.parent() {
            if parent.kind() == "source_file" {
                return true;
            }
            if parent.kind() == "class_body" {
                let class_body_range = crate::position::node_to_range(&parent);
                if is_position_in_range(target.location.range.start, class_body_range) {
                    return true;
                }
            }
        }
        let bind_range = crate::position::node_to_range(&binding);
        let is_target_decl = bind_range == target.location.range
            || is_position_in_range(target.location.range.start, bind_range);
        if !is_target_decl {
            return false;
        }
    }
    true
}

fn is_toplevel_target_visible(
    cand_url: &Url,
    target: &ReferenceTarget,
    target_pkg: Option<&str>,
    cand_pkg: Option<&str>,
    cand_file: Option<&crate::symbols::FileSymbols>,
) -> bool {
    cand_url == &target.location.uri
        || target_pkg.unwrap_or("") == cand_pkg.unwrap_or("")
        || cand_file.is_some_and(|file| {
            file.imports.iter().any(|imp| {
                super::filter::import_matches_target(imp, &target.name, target_pkg.unwrap_or(""))
            })
        })
}

fn verify_toplevel_target(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    cand_node: &Node<'_>,
    cand_url: &Url,
    source: &str,
    file_root: Option<Node<'_>>,
) -> bool {
    let target_file = index.files.get(&target.location.uri);
    let target_pkg = target_file.and_then(|file| file.package_name.as_deref());
    let cand_file = index.files.get(cand_url);
    let cand_pkg = cand_file.and_then(|file| file.package_name.as_deref());

    if let Some(receiver_node) = crate::syntax::ast_util::extract_receiver_node(cand_node) {
        if crate::inference::infer_node_type(index, cand_url, receiver_node, source, file_root)
            .is_some()
        {
            return false;
        }
        let nav_text = source
            .get(receiver_node.byte_range())
            .map(str::trim)
            .unwrap_or("");
        return target_pkg.is_some_and(|pkg| nav_text == pkg);
    }

    if !matches_lexical_binding(target, cand_node, cand_url, source) {
        return false;
    }

    if !is_toplevel_target_visible(cand_url, target, target_pkg, cand_pkg, cand_file) {
        return false;
    }

    verify_call_overload_signature(index, target, cand_node, cand_url, source, file_root)
}
