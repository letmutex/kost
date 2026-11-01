use ahash::AHashMap;
use tree_sitter::Node;

use super::util::{
    find_child_by_kind, node_text, resolve_kotlin_expr, resolve_kotlin_string,
    unpack_kotlin_value_argument,
};

pub(crate) fn extract_kotlin_properties(
    node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_declaration" {
            extract_property_declaration(child, source, properties);
        } else if child.kind() == "assignment" {
            extract_extra_index_assignment(child, source, properties);
        } else {
            extract_kotlin_properties(child, source, properties);
        }
    }
}

fn extract_property_declaration(
    node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
) {
    let mut var_name: Option<&str> = None;
    let mut var_val: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declaration" {
            if let Some(ident) = find_child_by_kind(child, "simple_identifier") {
                var_name = Some(node_text(ident, source));
            }
        } else if child.kind() == "property_delegate" {
            var_val = extract_delegate_string_value(child, source, properties);
        } else if child.kind() == "integer_literal" {
            var_val = Some(node_text(child, source).trim().to_string());
        } else if child.is_named()
            && let Some(resolved) = resolve_kotlin_expr(child, source, properties)
        {
            var_val = Some(resolved);
        }
    }

    if let (Some(name), Some(val)) = (var_name, var_val)
        && !name.is_empty()
    {
        properties.insert(name.to_string(), val);
    }
}

fn extract_delegate_string_value(
    delegate_node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
) -> Option<String> {
    let call_node = find_child_by_kind(delegate_node, "call_expression")?;
    let suffix = find_child_by_kind(call_node, "call_suffix")?;
    let val_args = find_child_by_kind(suffix, "value_arguments")?;
    let mut cursor = val_args.walk();
    for arg in val_args.children(&mut cursor) {
        if arg.kind() == "value_argument"
            && let Some((_, expr)) = unpack_kotlin_value_argument(arg, source)
            && let Some(resolved) = resolve_kotlin_expr(expr, source, properties)
        {
            return Some(resolved);
        }
    }
    None
}

fn extract_extra_index_assignment(
    assign_node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
) {
    let Some(lhs) = assign_node.child(0) else {
        return;
    };
    let Some(rhs) = assign_node.child(2).or_else(|| assign_node.child(1)) else {
        return;
    };
    let indexing_holder = find_child_by_kind(lhs, "indexing_expression").unwrap_or(lhs);
    let Some(target) = indexing_holder.child(0) else {
        return;
    };
    let target_text = node_text(target, source).trim();
    if target_text != "extra" && !target_text.ends_with(".extra") {
        return;
    }
    let Some(suffix) = find_child_by_kind(indexing_holder, "indexing_suffix") else {
        return;
    };
    let Some(key_node) = find_child_by_kind(suffix, "string_literal") else {
        return;
    };
    let key = resolve_kotlin_string(key_node, source, properties);
    if !key.is_empty()
        && let Some(val) = resolve_kotlin_expr(rhs, source, properties)
    {
        properties.insert(key, val);
    }
}
