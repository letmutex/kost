use std::collections::HashSet;
use tower_lsp::lsp_types::Position;
use tree_sitter::Node;

use crate::parser::{extract_identifier_text, find_node_at_position};

pub fn find_enclosing_class_name(node: Node, source: &str) -> Option<String> {
    let mut current = Some(node);
    while let Some(parent) = current {
        let k = parent.kind();
        if matches!(
            k,
            "class_declaration" | "object_declaration" | "companion_object"
        ) {
            return extract_identifier_text(&parent, source)
                .map(|s| s.to_string())
                .or_else(|| (k == "companion_object").then(|| "Companion".to_string()));
        }
        if let Some((name, _, _, _)) =
            crate::symbols::check_infix_class_or_interface(&parent, source)
        {
            return Some(name.to_string());
        }
        current = parent.parent();
    }
    None
}

pub fn collect_enclosing_classes(
    root: Option<Node>,
    pos: Position,
    source: &str,
) -> HashSet<String> {
    let mut classes = HashSet::new();
    let mut current = root.and_then(|r| find_node_at_position(r, pos));
    let mut chain = Vec::new();
    while let Some(curr) = current {
        let k = curr.kind();
        if matches!(
            k,
            "class_declaration" | "object_declaration" | "companion_object"
        ) {
            let name =
                extract_identifier_text(&curr, source).unwrap_or(if k == "companion_object" {
                    "Companion"
                } else {
                    ""
                });
            if !name.is_empty() {
                chain.push(name.to_string());
            }
        } else if let Some((name, _, _, _)) =
            crate::symbols::check_infix_class_or_interface(&curr, source)
        {
            chain.push(name.to_string());
        }
        current = curr.parent();
    }

    if !chain.is_empty() {
        chain.reverse();
        let mut full = String::new();
        for (i, seg) in chain.iter().enumerate() {
            if i > 0 {
                full.push('.');
            }
            full.push_str(seg);
            classes.insert(full.clone());
            classes.insert(common::concat_str!(&full, ".Companion"));
        }
    }

    classes
}

pub fn find_parameter<'a>(scope_node: Node<'a>, name: &str, source: &'a str) -> Option<Node<'a>> {
    let mut cursor = scope_node.walk();
    for child in scope_node.children(&mut cursor) {
        let k = child.kind();
        if matches!(k, "parameter" | "class_parameter")
            && extract_identifier_text(&child, source) == Some(name)
        {
            return Some(child);
        }
        if matches!(
            k,
            "function_value_parameters"
                | "class_parameters"
                | "primary_constructor"
                | "secondary_constructor"
                | "class_body"
        ) && let Some(found) = find_parameter(child, name, source)
        {
            return Some(found);
        }
    }
    None
}

fn check_multi_var_children<'a>(
    node: Node<'a>,
    name: &str,
    source: &'a str,
    before_byte: usize,
    role: super::scope::BindingRole,
) -> Option<Node<'a>> {
    if role == super::scope::BindingRole::DirectCall
        || role == super::scope::BindingRole::CallableRef
    {
        return None;
    }
    let mut multi_cursor = node.walk();
    node.children(&mut multi_cursor).find(|&var_child| {
        var_child.kind() == "variable_declaration"
            && (node.start_byte() <= before_byte
                || node.parent().is_some_and(|p| {
                    p.kind() == "lambda_parameters" || p.kind() == "for_statement"
                }))
            && extract_identifier_text(&var_child, source) == Some(name)
    })
}

pub(super) fn check_declaration_node<'a>(
    child: Node<'a>,
    name: &str,
    source: &'a str,
    before_byte: usize,
    role: super::scope::BindingRole,
) -> Option<Node<'a>> {
    let k = child.kind();
    if k == "property_declaration" {
        let mut cursor = child.walk();
        for grand_child in child.children(&mut cursor) {
            if grand_child.kind() == "variable_declaration"
                && child.start_byte() <= before_byte
                && role != super::scope::BindingRole::DirectCall
                && role != super::scope::BindingRole::CallableRef
                && extract_identifier_text(&grand_child, source) == Some(name)
            {
                return Some(grand_child);
            }
            if grand_child.kind() == "multi_variable_declaration"
                && let Some(found) =
                    check_multi_var_children(grand_child, name, source, before_byte, role)
            {
                return Some(found);
            }
        }
        return None;
    }
    if k == "multi_variable_declaration" {
        return check_multi_var_children(child, name, source, before_byte, role);
    }

    let is_catch_param = (k == "simple_identifier" || k == "catch_parameter")
        && child.parent().is_some_and(|p| p.kind() == "catch_block");
    let is_param = matches!(k, "parameter" | "class_parameter") || is_catch_param;
    let is_decl = is_param || matches!(k, "variable_declaration" | "function_declaration");

    if is_decl
        && (is_param || child.start_byte() <= before_byte)
        && extract_identifier_text(&child, source) == Some(name)
    {
        let matches_role = match role {
            super::scope::BindingRole::DirectCall | super::scope::BindingRole::CallableRef => {
                k == "function_declaration"
            }
            super::scope::BindingRole::Value => k != "function_declaration",
        };
        if matches_role {
            return Some(child);
        }
    }
    None
}

pub(super) fn check_param_or_constructor_child<'a>(
    child: Node<'a>,
    name: &str,
    source: &'a str,
    before_byte: usize,
    role: super::scope::BindingRole,
) -> Option<Node<'a>> {
    let ck = child.kind();
    if ck == "function_value_parameters" || ck == "lambda_parameters" || ck == "class_parameters" {
        let mut param_cursor = child.walk();
        for param in child.children(&mut param_cursor) {
            if let Some(found) = check_declaration_node(param, name, source, before_byte, role) {
                return Some(found);
            }
        }
    } else if ck == "primary_constructor" {
        let mut pc_cursor = child.walk();
        for pc_child in child.children(&mut pc_cursor) {
            if pc_child.kind() == "class_parameter" {
                if let Some(found) =
                    check_declaration_node(pc_child, name, source, before_byte, role)
                {
                    return Some(found);
                }
            } else if pc_child.kind() == "class_parameters" {
                let mut param_cursor = pc_child.walk();
                for param in pc_child.children(&mut param_cursor) {
                    if let Some(found) =
                        check_declaration_node(param, name, source, before_byte, role)
                    {
                        return Some(found);
                    }
                }
            }
        }
    } else if ck == "when_subject" {
        let mut ws_cursor = child.walk();
        for ws_child in child.children(&mut ws_cursor) {
            if let Some(found) = check_declaration_node(ws_child, name, source, before_byte, role) {
                return Some(found);
            }
        }
    }
    None
}
