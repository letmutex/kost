use smol_str::SmolStr;
use tree_sitter::Node;

use super::signature::GenericParamInfo;
use super::types::ast_type::is_type_node;
use super::types::{KotlinType, ast_node_to_kotlin_type};

struct RawTypeParam<'a> {
    name: SmolStr,
    is_out: bool,
    bound_node: Option<Node<'a>>,
}

pub fn extract_type_parameters<'a>(
    node: &Node<'a>,
    source: &'a str,
) -> (Vec<GenericParamInfo>, Vec<SmolStr>) {
    let mut param_names = Vec::new();
    let mut raw_params = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_parameters" {
            let mut type_cursor = child.walk();
            for param_child in child.children(&mut type_cursor) {
                if param_child.kind() == "type_parameter"
                    && let Some(type_param) = inspect_single_tp(&param_child, source)
                {
                    param_names.push(type_param.name.clone());
                    raw_params.push(type_param);
                }
            }
        }
    }

    let param_name_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();
    let mut type_params = Vec::new();
    for raw in raw_params {
        let mut bounds = Vec::new();
        if let Some(bound_node) = raw.bound_node
            && let Some(kotlin_type) =
                ast_node_to_kotlin_type(&bound_node, source, &param_name_refs, false)
        {
            bounds.push(kotlin_type);
        }
        type_params.push(GenericParamInfo {
            name: raw.name,
            bounds,
            is_out: raw.is_out,
        });
    }

    apply_where_constraints(node, source, &mut type_params, &param_name_refs);
    (type_params, param_names)
}

fn inspect_single_tp<'a>(node: &Node<'a>, source: &'a str) -> Option<RawTypeParam<'a>> {
    let mut name = SmolStr::default();
    let mut is_out = false;
    let mut bound_node = None;
    let mut is_after_colon = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let child_kind = child.kind();
        if child_kind == "type_identifier" && !is_after_colon {
            let text = source.get(child.start_byte()..child.end_byte())?;
            name = SmolStr::new(text.trim_matches('`'));
        } else if child_kind == "type_parameter_modifiers" {
            is_out = has_out_variance_modifier(&child, source);
        } else if child_kind == ":" {
            is_after_colon = true;
        } else if is_after_colon && is_type_node(&child) {
            bound_node = Some(child);
        }
    }
    if name.is_empty() {
        None
    } else {
        Some(RawTypeParam {
            name,
            is_out,
            bound_node,
        })
    }
}

fn has_out_variance_modifier(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variance_modifier"
            && let Some(text) = source.get(child.start_byte()..child.end_byte())
            && text == "out"
        {
            return true;
        }
    }
    false
}

fn apply_where_constraints(
    node: &Node,
    source: &str,
    type_params: &mut [GenericParamInfo],
    param_names: &[&str],
) {
    let mut where_cursor = node.walk();
    for child in node.children(&mut where_cursor) {
        if child.kind() != "type_constraints" {
            continue;
        }
        let mut constraint_cursor = child.walk();
        for constr in child.children(&mut constraint_cursor) {
            if constr.kind() != "type_constraint" {
                continue;
            }
            if let Some((target, bound)) = extract_constraint(&constr, source, param_names)
                && let Some(type_param) = type_params.iter_mut().find(|p| p.name.as_str() == target)
            {
                type_param.bounds.push(bound);
            }
        }
    }
}

fn extract_constraint<'a>(
    constr: &Node,
    source: &'a str,
    param_names: &[&str],
) -> Option<(&'a str, KotlinType)> {
    let mut param_target = None;
    let mut bound_type = None;
    let mut is_after_colon = false;
    let mut part_cursor = constr.walk();
    for part in constr.children(&mut part_cursor) {
        let part_kind = part.kind();
        if (part_kind == "type_identifier" || part_kind == "simple_identifier") && !is_after_colon {
            param_target = source
                .get(part.start_byte()..part.end_byte())
                .map(|s| s.trim_matches('`'));
        } else if part_kind == ":" {
            is_after_colon = true;
        } else if is_after_colon && is_type_node(&part) {
            bound_type = ast_node_to_kotlin_type(&part, source, param_names, false);
        }
    }
    Some((param_target?, bound_type?))
}
