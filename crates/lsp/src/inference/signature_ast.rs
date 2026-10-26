use smol_str::SmolStr;
use tree_sitter::Node;

use super::signature::{SignatureInfo, SignatureParam};
use super::signature_ast_generics::extract_type_parameters;
use super::types::ast_type::{has_suspend_modifier, is_type_node};
use super::types::{KotlinType, ast_node_to_kotlin_type};
use crate::syntax::parser::parse_source;

pub fn parse_signature_from_ast(source: &str) -> Option<SignatureInfo> {
    let clean = source.trim();
    if clean.is_empty() {
        return None;
    }
    if let Some(info) = parse_decl_from_source(clean) {
        return Some(info);
    }
    let fun_wrapped = common::concat_str!("fun ", clean);
    parse_decl_from_source(&fun_wrapped)
}

fn parse_decl_from_source(source: &str) -> Option<SignatureInfo> {
    let tree = parse_source(source)?;
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_declaration" => return parse_function_declaration(&child, source),
            "class_declaration" => return parse_class_declaration(&child, source),
            "secondary_constructor" => return parse_secondary_constructor(&child, source),
            "property_declaration" => return parse_property_declaration(&child, source),
            _ => {}
        }
    }
    None
}

fn parse_function_declaration(node: &Node, source: &str) -> Option<SignatureInfo> {
    let (type_params, param_names) = extract_type_parameters(node, source);
    let param_name_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();

    let mut receiver = None;
    let mut params = Vec::new();
    let mut return_type = None;
    let mut is_after_params = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "receiver_type" => {
                let mut receiver_cursor = child.walk();
                let mut is_suspend = false;
                for rchild in child.children(&mut receiver_cursor) {
                    if rchild.kind() == "type_modifiers" && has_suspend_modifier(&rchild) {
                        is_suspend = true;
                    } else if is_type_node(&rchild) {
                        receiver =
                            ast_node_to_kotlin_type(&rchild, source, &param_name_refs, is_suspend);
                        break;
                    }
                }
            }
            "function_value_parameters" => {
                is_after_params = true;
                params = extract_parameters(&child, source, &param_name_refs);
            }
            _ => {
                if is_after_params && is_type_node(&child) {
                    return_type = ast_node_to_kotlin_type(&child, source, &param_name_refs, false);
                }
            }
        }
    }

    Some(SignatureInfo {
        type_params,
        receiver,
        params,
        return_type,
    })
}

fn extract_parameters(node: &Node, source: &str, param_names: &[&str]) -> Vec<SignatureParam> {
    let mut params = Vec::new();
    let mut pending_vararg = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let child_kind = child.kind();
        if child_kind == "parameter_modifiers" {
            if has_vararg_modifier(&child, source) {
                pending_vararg = true;
            }
        } else if child_kind == "parameter" || child_kind == "class_parameter" {
            if let Some(mut param) = extract_single_param(&child, source, param_names) {
                if pending_vararg {
                    param.is_vararg = true;
                    pending_vararg = false;
                }
                params.push(param);
            }
        } else if child_kind == "=" {
            if let Some(last) = params.last_mut() {
                last.has_default = true;
            }
        } else if child_kind == "," {
            pending_vararg = false;
        }
    }
    params
}

fn extract_single_param(
    child: &Node,
    source: &str,
    param_names: &[&str],
) -> Option<SignatureParam> {
    let mut name = SmolStr::default();
    let mut is_vararg = false;
    let mut has_default = false;
    let mut colon_end = None;
    let mut eq_start = None;
    let mut type_node = None;
    let mut is_suspend = false;

    let mut part_cursor = child.walk();
    for part in child.children(&mut part_cursor) {
        match part.kind() {
            "simple_identifier" => {
                if name.is_empty() {
                    let text = source.get(part.start_byte()..part.end_byte())?;
                    name = SmolStr::new(text.trim_matches('`'));
                }
            }
            "parameter_modifiers" => {
                if has_vararg_modifier(&part, source) {
                    is_vararg = true;
                }
            }
            "type_modifiers" => {
                if has_suspend_modifier(&part) {
                    is_suspend = true;
                }
            }
            ":" => colon_end = Some(part.end_byte()),
            "=" => {
                has_default = true;
                eq_start = Some(part.start_byte());
            }
            _ => {
                if is_type_node(&part) {
                    type_node = Some(part);
                }
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    let (type_name, kotlin_type) = resolve_param_type(
        child,
        type_node,
        colon_end,
        eq_start,
        source,
        param_names,
        is_suspend,
    );

    Some(SignatureParam {
        name,
        type_name,
        kotlin_type,
        has_default,
        is_vararg,
    })
}

fn resolve_param_type(
    child: &Node,
    type_node: Option<Node>,
    colon_end: Option<usize>,
    eq_start: Option<usize>,
    source: &str,
    param_names: &[&str],
    is_suspend: bool,
) -> (SmolStr, Option<KotlinType>) {
    if let Some(type_ast_node) = type_node {
        let clean_text = if let Some(c_end) = colon_end {
            let t_end = eq_start.unwrap_or_else(|| child.end_byte());
            source.get(c_end..t_end).map(|s| s.trim()).unwrap_or("")
        } else {
            source
                .get(type_ast_node.start_byte()..type_ast_node.end_byte())
                .unwrap_or("")
                .trim()
        };
        (
            SmolStr::new(clean_text),
            ast_node_to_kotlin_type(&type_ast_node, source, param_names, is_suspend),
        )
    } else if let Some(c_end) = colon_end {
        let t_end = eq_start.unwrap_or_else(|| child.end_byte());
        if let Some(raw) = source.get(c_end..t_end) {
            let clean = raw.trim();
            (
                SmolStr::new(clean),
                super::types::parse_type_signature_with_params(clean, param_names),
            )
        } else {
            (SmolStr::default(), None)
        }
    } else {
        (SmolStr::default(), None)
    }
}

fn has_vararg_modifier(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameter_modifier"
            && let Some(text) = source.get(child.start_byte()..child.end_byte())
            && text == "vararg"
        {
            return true;
        }
    }
    false
}

fn parse_class_declaration(node: &Node, source: &str) -> Option<SignatureInfo> {
    let (type_params, param_names) = extract_type_parameters(node, source);
    let param_name_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();

    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "primary_constructor" {
            params = extract_parameters(&child, source, &param_name_refs);
        }
    }

    Some(SignatureInfo {
        type_params,
        receiver: None,
        params,
        return_type: None,
    })
}

fn parse_secondary_constructor(node: &Node, source: &str) -> Option<SignatureInfo> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            params = extract_parameters(&child, source, &[]);
        }
    }
    Some(SignatureInfo {
        type_params: Vec::new(),
        receiver: None,
        params,
        return_type: None,
    })
}

fn parse_property_declaration(node: &Node, source: &str) -> Option<SignatureInfo> {
    let (type_params, param_names) = extract_type_parameters(node, source);
    let param_name_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();

    let mut receiver = None;
    let mut return_type = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "receiver_type" => {
                let mut receiver_cursor = child.walk();
                for rchild in child.children(&mut receiver_cursor) {
                    if is_type_node(&rchild) {
                        receiver =
                            ast_node_to_kotlin_type(&rchild, source, &param_name_refs, false);
                        break;
                    }
                }
            }
            "variable_declaration" => {
                let mut var_cursor = child.walk();
                for vchild in child.children(&mut var_cursor) {
                    if is_type_node(&vchild) {
                        return_type =
                            ast_node_to_kotlin_type(&vchild, source, &param_name_refs, false);
                    }
                }
            }
            _ => {
                if is_type_node(&child) {
                    return_type = ast_node_to_kotlin_type(&child, source, &param_name_refs, false);
                }
            }
        }
    }

    Some(SignatureInfo {
        type_params,
        receiver,
        params: Vec::new(),
        return_type,
    })
}
