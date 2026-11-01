use ahash::AHashMap;
use tree_sitter::Node;

use super::util::{find_child_by_kind, node_text, resolve_groovy_expr};

/// Extracts declared local variables, ext assignments, and ext { ... } blocks.
pub(crate) fn extract_groovy_properties(
    root: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
) {
    extract_groovy_properties_scoped(root, source, properties, true);
}

fn extract_groovy_properties_scoped(
    node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
    allow_bare_assignment: bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "local_variable_declaration" => {
                extract_local_variable_declaration(child, source, properties);
            }
            "assignment_expression" => {
                extract_assignment_expression(child, source, properties, allow_bare_assignment);
            }
            "expression_statement" => {
                extract_groovy_properties_scoped(child, source, properties, allow_bare_assignment);
            }
            "command_chain" | "method_invocation" => {
                if is_ext_invocation(child, source) {
                    if let Some(closure) = find_invocation_closure(child) {
                        extract_groovy_properties_scoped(closure, source, properties, true);
                    }
                } else {
                    extract_groovy_properties_scoped(child, source, properties, false);
                }
            }
            "closure" | "argument_list" => {
                extract_groovy_properties_scoped(child, source, properties, allow_bare_assignment);
            }
            _ => {}
        }
    }
}

fn is_ext_invocation(node: Node, source: &[u8]) -> bool {
    let target = match node.kind() {
        "command_chain" => node
            .child_by_field_name("receiver")
            .or_else(|| node.child(0)),
        "method_invocation" => node
            .child_by_field_name("function")
            .or_else(|| node.child(0)),
        _ => None,
    };
    target.is_some_and(|target_node| {
        let name = node_text(target_node, source);
        name == "ext" || name.ends_with(".ext")
    })
}

fn find_invocation_closure(node: Node) -> Option<Node> {
    find_child_by_kind(node, "closure").or_else(|| {
        let arg_list = find_child_by_kind(node, "argument_list")?;
        find_child_by_kind(arg_list, "closure")
    })
}

fn extract_local_variable_declaration(
    node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }

        let name_node = child
            .child_by_field_name("name")
            .or_else(|| find_child_by_kind(child, "identifier"));
        let val_node = child.child_by_field_name("value").or_else(|| {
            let mut val_cursor = child.walk();
            child
                .children(&mut val_cursor)
                .filter(|sub| sub.is_named())
                .nth(1)
        });

        if let (Some(ident_node), Some(expr_node)) = (name_node, val_node) {
            let name = node_text(ident_node, source).trim();
            if !name.is_empty()
                && let Some(val) = resolve_groovy_expr(expr_node, source, properties)
            {
                properties.insert(name.to_string(), val);
            }
        }
    }
}

fn extract_assignment_expression(
    assign_node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
    allow_bare_assignment: bool,
) {
    let Some(left) = assign_node
        .child_by_field_name("left")
        .or_else(|| assign_node.child(0))
    else {
        return;
    };
    let Some(right) = assign_node
        .child_by_field_name("right")
        .or_else(|| assign_node.child(2))
    else {
        return;
    };

    let var_name = match left.kind() {
        "field_access" => {
            let field = left.child_by_field_name("field").or_else(|| {
                let mut cursor = left.walk();
                left.children(&mut cursor)
                    .filter(|child| child.kind() == "identifier")
                    .last()
            });
            field.map(|field_node| node_text(field_node, source))
        }
        "identifier" if allow_bare_assignment => Some(node_text(left, source)),
        _ => None,
    };

    if let Some(name) = var_name
        && !name.is_empty()
        && let Some(val) = resolve_groovy_expr(right, source, properties)
    {
        properties.insert(name.to_string(), val);
    }
}
