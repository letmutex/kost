use ahash::AHashMap;
use tree_sitter::Node;

use super::kotlin::push_extracted_coordinates;
use super::util::{is_dependency_configuration, node_text, resolve_groovy_expr};
use crate::coordinate::DependencyCoordinate;

pub(crate) fn extract_groovy_subscript_dependency(
    node: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) -> bool {
    let inner = if node.kind() == "expression_statement" {
        node.child(0).unwrap_or(node)
    } else {
        node
    };
    if inner.kind() != "subscript_expression" {
        return false;
    }
    let Some(obj) = inner
        .child_by_field_name("object")
        .or_else(|| inner.child(0))
    else {
        return false;
    };
    if !is_dependency_configuration(node_text(obj, source).trim()) {
        return false;
    }

    let mut named_group = None;
    let mut named_name = None;
    let mut named_ver = None;
    let mut positional_vals = Vec::new();

    let mut elements = Vec::new();
    collect_subscript_elements(inner, &mut elements);

    let mut idx = 1;
    while idx < elements.len() {
        let elem = elements[idx];
        let key = node_text(elem, source).trim().trim_matches(['\'', '"']);
        if matches!(key, "group" | "name" | "version") && idx + 1 < elements.len() {
            let val_node = elements[idx + 1];
            if let Some(val) = resolve_groovy_expr(val_node, source, vars) {
                match key {
                    "group" => named_group = Some(val),
                    "name" => named_name = Some(val),
                    "version" => named_ver = Some(val),
                    _ => {}
                }
            }
            idx += 2;
            continue;
        }
        if let Some(val) = resolve_groovy_expr(elem, source, vars) {
            positional_vals.push(val);
        }
        idx += 1;
    }

    push_extracted_coordinates(positional_vals, named_group, named_name, named_ver, deps);
    true
}

fn collect_subscript_elements<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "ERROR" {
            collect_subscript_elements(child, out);
        } else if child.is_named() {
            out.push(child);
        }
    }
}
