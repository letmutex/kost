use tower_lsp::lsp_types::{Location, SymbolKind};
use tree_sitter::Node;

use super::target::ReferenceTarget;
use crate::indexer::WorkspaceIndex;
use crate::position::node_to_range;
use crate::syntax::hierarchy::containers_match;

pub fn is_in_container_hierarchy(
    index: &WorkspaceIndex,
    container_a: &str,
    container_b: &str,
) -> bool {
    let clean_a = container_a
        .split(['<', '?'])
        .next()
        .unwrap_or(container_a)
        .trim();
    let clean_b = container_b
        .split(['<', '?'])
        .next()
        .unwrap_or(container_b)
        .trim();

    if containers_match(clean_a, clean_b) {
        return true;
    }

    if crate::inference::types::StandardInterface::from_name(clean_b)
        == Some(crate::inference::types::StandardInterface::Any)
    {
        return true;
    }

    if let Some(prim_a) = crate::inference::model::PrimitiveType::from_name(clean_a)
        && let Some(std_b) = crate::inference::types::StandardInterface::from_name(clean_b)
        && std_b.conforms_primitive(prim_a)
    {
        return true;
    }

    if crate::inference::model::PrimitiveType::from_name(clean_a)
        == Some(crate::inference::model::PrimitiveType::Nothing)
    {
        return true;
    }

    if index.is_subclass_of(clean_a, clean_b) {
        return true;
    }

    false
}

fn is_companion_object(index: &WorkspaceIndex, target_container: &str) -> bool {
    let Some((enclosing, name)) = target_container.rsplit_once('.') else {
        return false;
    };
    if name == "Companion" {
        return true;
    }
    index
        .container_to_decls
        .get(enclosing)
        .is_some_and(|decls| {
            decls.iter().any(|(_, decl)| {
                decl.name == name
                    && decl.kind == tower_lsp::lsp_types::SymbolKind::OBJECT
                    && decl.signature.as_deref().is_some_and(|sig| {
                        sig.starts_with("companion") || sig.contains("companion object")
                    })
            })
        })
}

pub fn conforms_to_container_or_companion(
    index: &WorkspaceIndex,
    actual_type: &str,
    target_container: &str,
) -> bool {
    if is_in_container_hierarchy(index, actual_type, target_container) {
        return true;
    }
    let clean = target_container
        .strip_suffix(".Companion")
        .unwrap_or(target_container);
    if is_in_container_hierarchy(index, actual_type, clean) {
        return true;
    }
    if let Some((enclosing, _)) = target_container.rsplit_once('.')
        && is_companion_object(index, target_container)
        && is_in_container_hierarchy(index, actual_type, enclosing)
    {
        return true;
    }
    false
}

pub fn check_candidate_node(
    index: &WorkspaceIndex,
    include_declaration: bool,
    target: &ReferenceTarget,
    cand_url: &tower_lsp::lsp_types::Url,
    cand_node: Node<'_>,
    source: &str,
    file_root: Node<'_>,
    results: &mut Vec<Location>,
) {
    let cand_range = node_to_range(&cand_node);
    if cand_url == &target.location.uri && cand_range == target.location.range {
        if include_declaration {
            results.push(Location {
                uri: cand_url.clone(),
                range: cand_range,
            });
        }
        return;
    }

    if super::target::is_declaration_name_node(&cand_node) {
        if include_declaration
            && verify_override_hierarchy_candidate(index, target, &cand_node, source)
        {
            results.push(Location {
                uri: cand_url.clone(),
                range: cand_range,
            });
        }
        return;
    }

    if super::verify::verify_candidate_target(
        index,
        target,
        &cand_node,
        cand_url,
        source,
        Some(file_root),
    ) {
        results.push(Location {
            uri: cand_url.clone(),
            range: cand_range,
        });
    }
}

pub fn verify_override_hierarchy_candidate(
    index: &WorkspaceIndex,
    target: &ReferenceTarget,
    cand_node: &Node<'_>,
    source: &str,
) -> bool {
    let Some(target_container) = target.container_name.as_deref() else {
        return false;
    };
    if target.is_private || target.is_local {
        return false;
    }

    let Some(mut decl_node) = cand_node.parent() else {
        return false;
    };
    if decl_node.kind() == "variable_declaration"
        && let Some(parent_decl) = decl_node.parent()
        && parent_decl.kind() == "property_declaration"
    {
        decl_node = parent_decl;
    }

    if !matches!(
        decl_node.kind(),
        "function_declaration" | "property_declaration" | "class_parameter"
    ) {
        return false;
    }

    if decl_node.kind() == "class_parameter"
        && !crate::parser::is_class_parameter_property(&decl_node)
    {
        return false;
    }

    if crate::parser::is_private_declaration(&decl_node, source) {
        return false;
    }

    let Some(cand_container) =
        crate::syntax::scope_enclosing::find_enclosing_class_name(decl_node, source)
    else {
        return false;
    };

    if crate::syntax::hierarchy::containers_match(&cand_container, target_container) {
        return false;
    }

    if !are_decl_kinds_compatible(&decl_node, target.kind) {
        return false;
    }

    if !check_function_signature_match(&decl_node, target.signature.as_deref(), source) {
        return false;
    }

    let cand_has_override = crate::parser::has_override_modifier(&decl_node, source);

    if cand_has_override && index.is_subclass_of(&cand_container, target_container) {
        return true;
    }

    if target.is_override && index.is_subclass_of(target_container, &cand_container) {
        return true;
    }

    false
}

fn are_decl_kinds_compatible(decl_node: &Node, target_kind: Option<SymbolKind>) -> bool {
    let Some(target_kind) = target_kind else {
        return true;
    };
    let is_cand_func = decl_node.kind() == "function_declaration";
    let is_target_func = matches!(
        target_kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
    );
    is_cand_func == is_target_func
}

fn check_function_signature_match(
    decl_node: &Node,
    target_sig: Option<&str>,
    source: &str,
) -> bool {
    if decl_node.kind() != "function_declaration" {
        return true;
    }
    let Some(target_sig) = target_sig else {
        return true;
    };
    let target_params = crate::syntax::signature::params::parse_signature_params(target_sig);
    let target_type_names: Vec<&str> = target_params
        .iter()
        .map(|param| param.type_name.as_str())
        .collect();
    let cand_type_names = extract_function_param_types(decl_node, source);
    let type_params: Vec<smol_str::SmolStr> =
        crate::syntax::signature::type_params::parse_signature_type_params(target_sig)
            .into_iter()
            .map(|param| param.name)
            .collect();
    are_override_param_types_compatible(&target_type_names, &cand_type_names, &type_params)
}

fn extract_function_param_types<'a>(decl_node: &Node<'a>, source: &'a str) -> Vec<&'a str> {
    let mut cursor = decl_node.walk();
    for child in decl_node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            let mut param_cursor = child.walk();
            return child
                .children(&mut param_cursor)
                .filter(|node| node.kind() == "parameter")
                .map(|param_node| {
                    crate::parser::extract_type_annotation(&param_node, source).unwrap_or_default()
                })
                .collect();
        }
    }
    Vec::new()
}

fn extract_type_simple_name(type_str: &str) -> &str {
    let clean = type_str.split(['<', '?']).next().unwrap_or(type_str).trim();
    clean.rsplit('.').next().unwrap_or(clean)
}

fn are_override_param_types_compatible(
    target_types: &[&str],
    cand_types: &[&str],
    type_params: &[smol_str::SmolStr],
) -> bool {
    if target_types.len() != cand_types.len() {
        return false;
    }
    target_types
        .iter()
        .zip(cand_types.iter())
        .all(|(&target_param, &cand_param)| {
            if target_param.is_empty() || cand_param.is_empty() {
                return true;
            }
            let (target_clean, cand_clean) = (
                target_param.trim_end_matches('?').trim(),
                cand_param.trim_end_matches('?').trim(),
            );
            let target_simple = extract_type_simple_name(target_clean);
            let cand_simple = extract_type_simple_name(cand_clean);
            if target_clean == cand_clean || target_simple == cand_simple {
                return true;
            }
            let is_any_target =
                crate::inference::types::StandardInterface::from_name(target_simple)
                    == Some(crate::inference::types::StandardInterface::Any);
            let is_any_cand = crate::inference::types::StandardInterface::from_name(cand_simple)
                == Some(crate::inference::types::StandardInterface::Any);
            if is_any_target && is_any_cand {
                return true;
            }
            type_params
                .iter()
                .any(|param| param.as_str() == target_simple || param.as_str() == cand_simple)
        })
}
