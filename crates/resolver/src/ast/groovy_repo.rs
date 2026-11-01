use ahash::AHashMap;
use tree_sitter::Node;

use super::groovy::parse_groovy_script_with_repos;
use super::util::{find_child_by_kind, is_repo_url, node_text, resolve_groovy_expr};
use super::{ParseError, RepositoriesResult};

/// Parse repository declarations from Groovy DSL script content.
pub fn parse_groovy_repositories(source: &str) -> Result<RepositoriesResult, ParseError> {
    parse_groovy_repositories_with_props(source, &AHashMap::default())
}

/// Parse repository declarations from Groovy DSL script content with inherited properties.
pub fn parse_groovy_repositories_with_props(
    source: &str,
    properties: &AHashMap<String, String>,
) -> Result<RepositoriesResult, ParseError> {
    let mut result = RepositoriesResult::default();
    parse_groovy_script_with_repos(source, properties, None, Some(&mut result))?;
    Ok(result)
}

pub(crate) fn extract_groovy_repos_node(
    node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    match node.kind() {
        "method_invocation" => {
            if let Some(function) = node
                .child_by_field_name("function")
                .or_else(|| node.child(0))
            {
                let func_name = node_text(function, source);
                match func_name {
                    "google" => result.google = true,
                    "mavenCentral" => result.maven_central = true,
                    "gradlePluginPortal" => result.gradle_plugin_portal = true,
                    "mavenLocal" => result.maven_local = true,
                    "maven" => {
                        extract_groovy_maven_call(node, source, props, result);
                        return;
                    }
                    _ => {}
                }
            }
        }
        "command_chain" => {
            if let Some(receiver) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
                && node_text(receiver, source) == "maven"
            {
                extract_groovy_maven_call(node, source, props, result);
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_repos_node(child, source, props, result);
    }
}

fn extract_groovy_maven_call(
    call_or_cmd: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    if call_or_cmd.kind() == "command_chain" {
        let mut cursor = call_or_cmd.walk();
        for arg in call_or_cmd.children(&mut cursor).skip(1) {
            if arg.kind() == "closure" {
                extract_groovy_maven_closure_body(arg, source, props, result);
            } else if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
                result.urls.push(url);
            }
        }
    } else if call_or_cmd.kind() == "method_invocation"
        && let Some(arg_list) = find_child_by_kind(call_or_cmd, "argument_list")
    {
        let mut cursor = arg_list.walk();
        for arg in arg_list.children(&mut cursor) {
            if arg.kind() == "closure" {
                extract_groovy_maven_closure_body(arg, source, props, result);
            } else if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
                result.urls.push(url);
            }
        }
    }
}

fn extract_url_from_groovy_expr(
    expr: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
) -> Option<String> {
    if let Some(url) = resolve_groovy_expr(expr, source, props)
        && is_repo_url(&url)
    {
        return Some(url);
    }

    match expr.kind() {
        "named_argument" | "map_entry" => {
            let mut cursor = expr.walk();
            let mut named_children = expr.children(&mut cursor).filter(|child| child.is_named());
            let key_node = named_children.next()?;
            let val_node = named_children.next()?;
            if node_text(key_node, source).trim_matches(['\'', '"']) == "url" {
                return extract_url_from_groovy_expr(val_node, source, props);
            }
        }
        "method_invocation" => {
            let func = expr
                .child_by_field_name("function")
                .or_else(|| expr.child(0))?;
            let func_name = node_text(func, source);
            if func_name == "uri" || func_name.ends_with("URI.create") {
                let arg_list = find_child_by_kind(expr, "argument_list")?;
                let mut cursor = arg_list.walk();
                for arg in arg_list.children(&mut cursor) {
                    if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
                        return Some(url);
                    }
                }
            }
        }
        "command_chain" => {
            let rec = expr
                .child_by_field_name("receiver")
                .or_else(|| expr.child(0))?;
            if node_text(rec, source) == "uri" {
                let arg = expr
                    .child_by_field_name("argument")
                    .or_else(|| expr.child(1))?;
                return extract_url_from_groovy_expr(arg, source, props);
            }
        }
        _ => {}
    }
    None
}

fn extract_groovy_maven_closure_body(
    closure: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    let mut cursor = closure.walk();
    for child in closure.children(&mut cursor) {
        match child.kind() {
            "command_chain" => {
                extract_url_from_command_chain(child, source, props, result);
            }
            "expression_statement" => {
                extract_groovy_maven_closure_body(child, source, props, result);
            }
            "assignment_expression" => {
                extract_url_from_assignment_expr(child, source, props, result);
            }
            "method_invocation" => {
                extract_url_from_method_invocation(child, source, props, result);
            }
            _ => {}
        }
    }
}

fn extract_url_from_command_chain(
    child: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    let Some(rec) = child
        .child_by_field_name("receiver")
        .or_else(|| child.child(0))
    else {
        return;
    };
    if node_text(rec, source) != "url" {
        return;
    }
    let Some(arg) = child
        .child_by_field_name("argument")
        .or_else(|| child.child(1))
    else {
        return;
    };
    if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
        result.urls.push(url);
    }
}

fn extract_url_from_assignment_expr(
    child: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    let Some(left) = child.child_by_field_name("left").or_else(|| child.child(0)) else {
        return;
    };
    if node_text(left, source).trim() != "url" {
        return;
    }
    let Some(right) = child
        .child_by_field_name("right")
        .or_else(|| child.child(2))
    else {
        return;
    };
    if let Some(url) = extract_url_from_groovy_expr(right, source, props) {
        result.urls.push(url);
    }
}

fn extract_url_from_method_invocation(
    child: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    let Some(func) = child
        .child_by_field_name("function")
        .or_else(|| child.child(0))
    else {
        return;
    };
    let func_name = node_text(func, source);
    if matches!(func_name, "url" | "setUrl") {
        let Some(arg_list) = find_child_by_kind(child, "argument_list") else {
            return;
        };
        let mut arg_cursor = arg_list.walk();
        for arg in arg_list.children(&mut arg_cursor) {
            if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
                result.urls.push(url);
            }
        }
    } else if (func_name == "uri" || func_name.ends_with("URI.create"))
        && let Some(url) = extract_url_from_groovy_expr(child, source, props)
    {
        result.urls.push(url);
    }
}
