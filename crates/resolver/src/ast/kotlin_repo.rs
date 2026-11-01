use ahash::AHashMap;
use tree_sitter::Node;

use super::kotlin::parse_kotlin_script_with_repos;
use super::util::{
    find_child_by_kind, is_repo_url, node_text, resolve_kotlin_expr, unpack_kotlin_value_argument,
};
use super::{ParseError, RepositoriesResult};

/// Parse repository declarations from Kotlin DSL script content.
pub fn parse_kotlin_repositories(source: &str) -> Result<RepositoriesResult, ParseError> {
    parse_kotlin_repositories_with_props(source, &AHashMap::default())
}

/// Parse repository declarations from Kotlin DSL script content with inherited properties.
pub fn parse_kotlin_repositories_with_props(
    source: &str,
    properties: &AHashMap<String, String>,
) -> Result<RepositoriesResult, ParseError> {
    let mut result = RepositoriesResult::default();
    parse_kotlin_script_with_repos(source, properties, None, Some(&mut result))?;
    Ok(result)
}

pub(crate) fn extract_kotlin_repos_node(
    node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    if node.kind() == "call_expression"
        && let Some(callee) = node.child(0)
    {
        let callee_name = node_text(callee, source);
        match callee_name {
            "google" => result.google = true,
            "mavenCentral" => result.maven_central = true,
            "gradlePluginPortal" => result.gradle_plugin_portal = true,
            "mavenLocal" => result.maven_local = true,
            "maven" => {
                extract_kotlin_maven_call(node, source, props, result);
                return;
            }
            _ => {}
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_kotlin_repos_node(child, source, props, result);
    }
}

fn extract_kotlin_maven_call(
    call_node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    let Some(call_suffix) = find_child_by_kind(call_node, "call_suffix") else {
        return;
    };

    if let Some(val_args) = find_child_by_kind(call_suffix, "value_arguments") {
        let mut cursor = val_args.walk();
        for arg in val_args.children(&mut cursor) {
            if arg.kind() == "value_argument"
                && let Some(url) = extract_url_from_argument(arg, source, props)
            {
                result.urls.push(url);
            }
        }
    }

    let Some(statements) = find_child_by_kind(call_suffix, "annotated_lambda")
        .and_then(|annotated_lambda| find_child_by_kind(annotated_lambda, "lambda_literal"))
        .and_then(|lambda_literal| find_child_by_kind(lambda_literal, "statements"))
    else {
        return;
    };

    let mut cursor = statements.walk();
    for stmt in statements.children(&mut cursor) {
        extract_url_from_lambda_stmt(stmt, source, props, result);
    }
}

fn extract_url_from_lambda_stmt(
    stmt: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
) {
    if stmt.kind() == "assignment" {
        let Some(lhs) = stmt.child(0) else {
            return;
        };
        if node_text(lhs, source).trim() != "url" {
            return;
        }
        let Some(rhs) = stmt.child(2).or_else(|| stmt.child(1)) else {
            return;
        };
        if let Some(url) = extract_url_from_expr(rhs, source, props) {
            result.urls.push(url);
        }
    } else if stmt.kind() == "call_expression" {
        let Some(callee) = stmt.child(0) else {
            return;
        };
        let callee_name = node_text(callee, source);
        if !matches!(callee_name, "url" | "setUrl") {
            return;
        }
        let Some(suffix) = find_child_by_kind(stmt, "call_suffix") else {
            return;
        };
        let Some(val_args) = find_child_by_kind(suffix, "value_arguments") else {
            return;
        };
        let mut arg_cursor = val_args.walk();
        for arg in val_args.children(&mut arg_cursor) {
            if arg.kind() == "value_argument"
                && let Some(url) = extract_url_from_argument(arg, source, props)
            {
                result.urls.push(url);
            }
        }
    }
}

fn extract_url_from_argument(
    arg_node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
) -> Option<String> {
    let (param_name, val_expr) = unpack_kotlin_value_argument(arg_node, source)?;
    if let Some(key) = param_name
        && key != "url"
    {
        return None;
    }
    extract_url_from_expr(val_expr, source, props)
}

fn extract_url_from_expr(
    expr_node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
) -> Option<String> {
    if let Some(val) = resolve_kotlin_expr(expr_node, source, props)
        && is_repo_url(&val)
    {
        return Some(val);
    }
    if expr_node.kind() == "call_expression" {
        let suffix = find_child_by_kind(expr_node, "call_suffix")?;
        let val_args = find_child_by_kind(suffix, "value_arguments")?;
        let mut cursor = val_args.walk();
        for arg in val_args.children(&mut cursor) {
            if arg.kind() == "value_argument"
                && let Some(url) = extract_url_from_argument(arg, source, props)
            {
                return Some(url);
            }
        }
    }
    None
}
