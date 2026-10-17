use tower_lsp::lsp_types::SymbolKind;
use tree_sitter::Node;

use super::hierarchy::is_in_container_hierarchy;
use super::target::ReferenceTarget;
use crate::features::definition::{BindingRole, find_local_binding_node};
use crate::indexer::WorkspaceIndex;
use crate::inference::model::PrimitiveType;
use crate::parser::extract_identifier_text;

use crate::inference::ast_literal::{infer_literal_kind_and_text, is_literal_kind};
use crate::inference::types::KotlinType;

fn is_literal_incompatible_with_container(
    index: &WorkspaceIndex,
    literal_kind: &str,
    literal_text: &str,
    target_container: &str,
) -> bool {
    let clean_target = target_container
        .strip_suffix(".Companion")
        .unwrap_or(target_container);
    let simple_target = clean_target.rsplit('.').next().unwrap_or(clean_target);
    if simple_target == PrimitiveType::Any.as_str() {
        return false;
    }

    let inferred_type = infer_literal_kind_and_text(literal_kind, literal_text);
    let (literal_type, primitive_type) = match inferred_type {
        Some(KotlinType::Primitive(prim)) => (prim.as_str(), Some(prim)),
        Some(KotlinType::Class { ref name, .. }) => (name.as_str(), None),
        _ => return false,
    };

    if let Some(standard_interface) =
        crate::inference::types::StandardInterface::from_name(simple_target)
        && let Some(primitive) = primitive_type
        && standard_interface.conforms_primitive(primitive)
    {
        return false;
    }

    !is_in_container_hierarchy(index, literal_type, target_container)
}

fn resolve_companion_target_names(target_container: &str) -> (Option<&str>, &str) {
    if let Some((enclosing, companion_name)) = target_container.rsplit_once('.') {
        let simple_enclosing = enclosing.rsplit('.').next().unwrap_or(enclosing);
        (Some(simple_enclosing), companion_name)
    } else {
        (None, target_container)
    }
}

fn is_target_container_object(index: &WorkspaceIndex, target_container: &str) -> bool {
    let (_, object_name) = resolve_companion_target_names(target_container);
    if let Some(decls) = index.name_to_decls.get(object_name) {
        return decls
            .iter()
            .any(|(_, decl)| decl.kind == SymbolKind::OBJECT);
    }
    false
}

fn is_known_class_or_object(index: &WorkspaceIndex, name: &str) -> bool {
    index.name_to_decls.get(name).is_some_and(|decls| {
        decls.iter().any(|(_, decl)| {
            matches!(
                decl.kind,
                SymbolKind::CLASS | SymbolKind::OBJECT | SymbolKind::INTERFACE
            )
        })
    })
}

fn is_object_receiver_incompatible(
    index: &WorkspaceIndex,
    receiver_node: &Node<'_>,
    source: &str,
    target_container: &str,
) -> bool {
    let (companion_enclosing, object_name) = resolve_companion_target_names(target_container);
    let is_matching_identifier = |text: &str| {
        text == "this"
            || text == object_name
            || companion_enclosing.is_some_and(|enclosing| text == enclosing)
    };

    if let Some(text) = extract_identifier_text(receiver_node, source) {
        if is_matching_identifier(text) {
            return false;
        }
        if is_known_class_or_object(index, text) {
            return true;
        }
    }

    if receiver_node.kind() == "navigation_expression" {
        let count = receiver_node.child_count();
        if count > 0
            && let Some(suffix) = receiver_node.child(count - 1)
            && let Some(suffix_id) = extract_identifier_text(&suffix, source)
        {
            if is_matching_identifier(suffix_id) {
                return false;
            }
            if is_known_class_or_object(index, suffix_id) {
                return true;
            }
        }
    }

    false
}

fn extract_type_from_node<'a>(node: Node<'a>, source: &'a str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "user_type" => {
                let mut type_cursor = child.walk();
                let mut ident_parts = Vec::new();
                for type_child in child.children(&mut type_cursor) {
                    if type_child.kind() == "type_identifier"
                        && let Some(ident_text) = extract_identifier_text(&type_child, source)
                    {
                        ident_parts.push(ident_text);
                    }
                }
                if !ident_parts.is_empty() {
                    return Some(ident_parts.join("."));
                }
            }
            "type_reference" | "nullable_type" | "variable_declaration" => {
                if let Some(found) = extract_type_from_node(child, source) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn find_local_receiver_explicit_type(
    receiver_node: &Node<'_>,
    source: &str,
) -> Option<String> {
    let receiver_name = extract_identifier_text(receiver_node, source)?;
    let binding_node =
        find_local_binding_node(*receiver_node, receiver_name, source, BindingRole::Value)?;

    extract_type_from_node(binding_node, source)
}

pub fn is_receiver_incompatible(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    receiver_node: &Node<'_>,
    source: &str,
) -> bool {
    let Some(ref target_container) = target.container_name else {
        return false;
    };

    let receiver_kind = receiver_node.kind();

    if is_literal_kind(receiver_kind) {
        let literal_text = source
            .get(receiver_node.start_byte()..receiver_node.end_byte())
            .unwrap_or_default();
        return is_literal_incompatible_with_container(
            index,
            receiver_kind,
            literal_text,
            target_container,
        );
    }

    if is_target_container_object(index, target_container) {
        return is_object_receiver_incompatible(index, receiver_node, source, target_container);
    }

    if receiver_kind == "simple_identifier"
        && let Some(explicit_type) = find_local_receiver_explicit_type(receiver_node, source)
        && !is_in_container_hierarchy(index, &explicit_type, target_container)
    {
        return true;
    }

    false
}
