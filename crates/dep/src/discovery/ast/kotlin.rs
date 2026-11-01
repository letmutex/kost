use ahash::AHashMap;
use tree_sitter::Node;

use super::kotlin_repo::extract_kotlin_repos_node;
use super::util::{
    find_child_by_kind, is_dependency_configuration, node_text, resolve_kotlin_expr,
    resolve_kotlin_string, unpack_kotlin_value_argument,
};
use super::{ParseError, RepositoriesResult};
use crate::coordinate::DependencyCoordinate;

thread_local! {
    pub(crate) static KOTLIN_PARSER: std::cell::RefCell<tree_sitter::Parser> = std::cell::RefCell::new({
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language =
            unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
        parser
            .set_language(&language)
            .expect("Failed to load Kotlin tree-sitter grammar");
        parser
    });
}

/// Parse dependencies and property declarations from Kotlin script content.
pub fn parse_kotlin_script(
    source: &str,
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) -> Result<(), ParseError> {
    parse_kotlin_script_with_repos(source, properties, Some(deps), None)
}

/// Parse Kotlin script once, extracting properties, optional dependencies, and optional repositories.
pub(crate) fn parse_kotlin_script_with_repos(
    source: &str,
    properties: &AHashMap<String, String>,
    deps: Option<&mut Vec<DependencyCoordinate>>,
    repos: Option<&mut RepositoriesResult>,
) -> Result<(), ParseError> {
    let source_bytes = source.as_bytes();
    let tree = KOTLIN_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));
    let Some(tree) = tree else {
        return Err(ParseError::TreeSitterError(
            "Failed to parse Kotlin script with tree-sitter".to_string(),
        ));
    };

    let root = tree.root_node();
    if root.has_error() {
        return Err(ParseError::SyntaxError(
            "Kotlin script contains syntax errors".to_string(),
        ));
    }

    let mut active_props = properties.clone();
    extract_kotlin_properties(root, source_bytes, &mut active_props);
    if let Some(deps_out) = deps {
        extract_kotlin_dependencies(root, source_bytes, &active_props, deps_out);
    }
    if let Some(repos_out) = repos {
        extract_kotlin_repos_node(root, source_bytes, &active_props, repos_out);
    }
    Ok(())
}

pub(crate) fn extract_kotlin_properties(
    node: Node,
    source: &[u8],
    properties: &mut AHashMap<String, String>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_declaration" {
            extract_property_declaration(child, source, properties);
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

fn extract_kotlin_dependencies(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    if node.kind() == "call_expression" {
        if let Some(callee) = node.child(0) {
            let callee_name = node_text(callee, source).trim_matches('"');
            if is_dependency_configuration(callee_name) {
                extract_call_dependency(node, source, properties, deps);
                return;
            }
        }
    } else if node.kind() == "infix_expression"
        && let Some(operator_node) = node.child(1)
    {
        let op_name = node_text(operator_node, source);
        if is_dependency_configuration(op_name) {
            extract_infix_dependency(node, source, properties, deps);
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_kotlin_dependencies(child, source, properties, deps);
    }
}

fn extract_infix_dependency(
    infix_node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    let Some(rhs) = infix_node.child(2) else {
        return;
    };
    if let Some(raw_coord) = resolve_kotlin_expr(rhs, source, properties).or_else(|| {
        (rhs.kind() == "string_literal").then(|| resolve_kotlin_string(rhs, source, properties))
    }) && let Some(coord) = DependencyCoordinate::parse(&raw_coord)
    {
        deps.push(coord);
    }
}

fn extract_call_dependency(
    call_node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    let Some(call_suffix) = find_child_by_kind(call_node, "call_suffix") else {
        return;
    };
    let Some(val_args) = find_child_by_kind(call_suffix, "value_arguments") else {
        return;
    };

    let mut named_group: Option<String> = None;
    let mut named_name: Option<String> = None;
    let mut named_ver: Option<String> = None;
    let mut positional_vals: Vec<String> = Vec::new();

    let mut cursor = val_args.walk();
    for arg in val_args.children(&mut cursor) {
        if arg.kind() != "value_argument" {
            continue;
        }
        let Some((param_name, val_expr)) = unpack_kotlin_value_argument(arg, source) else {
            continue;
        };

        if let Some(key) = param_name {
            if let Some(val) = resolve_kotlin_expr(val_expr, source, properties) {
                match key {
                    "group" => named_group = Some(val),
                    "name" => named_name = Some(val),
                    "version" => named_ver = Some(val),
                    _ => {}
                }
            }
            continue;
        }

        if val_expr.kind() == "call_expression" {
            if let Some(inner_callee) = val_expr.child(0)
                && matches!(
                    node_text(inner_callee, source),
                    "platform" | "enforcedPlatform"
                )
            {
                extract_call_dependency(val_expr, source, properties, deps);
            }
            continue;
        }

        if let Some(val) = resolve_kotlin_expr(val_expr, source, properties) {
            positional_vals.push(val);
        }
    }

    push_extracted_coordinates(positional_vals, named_group, named_name, named_ver, deps);
}

pub(crate) fn push_extracted_coordinates(
    positional_vals: Vec<String>,
    named_group: Option<String>,
    named_name: Option<String>,
    named_ver: Option<String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    if positional_vals.len() >= 2 && !positional_vals[0].contains(':') {
        deps.push(DependencyCoordinate::new(
            &positional_vals[0],
            &positional_vals[1],
            positional_vals.get(2).map(String::as_str),
        ));
    } else {
        for raw_coord in &positional_vals {
            if let Some(coord) = DependencyCoordinate::parse(raw_coord) {
                deps.push(coord);
            }
        }
    }

    if let (Some(group), Some(name)) = (named_group, named_name) {
        deps.push(DependencyCoordinate::new(group, name, named_ver.as_deref()));
    }
}
