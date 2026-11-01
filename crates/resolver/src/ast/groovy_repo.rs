use ahash::AHashMap;
use tree_sitter::Node;

use super::groovy::parse_groovy_script_with_repos;
use super::util::{
    find_child_by_kind, groovy_call_target, is_repo_url, node_text, resolve_groovy_expr,
};
use super::{DeclaredRepository, ParseError, RepositoriesResult};

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
    extract_groovy_repos_scoped(node, source, props, result, false, false);
}

fn extract_groovy_repos_scoped(
    node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
    in_repos_block: bool,
    in_plugin_scope: bool,
) {
    let mut next_repos = in_repos_block;
    let mut next_plugin = in_plugin_scope;

    match node.kind() {
        "closure" if in_repos_block => {
            extract_groovy_repos_closure(node, source, props, result, in_plugin_scope);
            return;
        }
        "method_invocation" | "command_chain" => {
            if let Some(target) = groovy_call_target(node) {
                let target_name = node_text(target, source).trim();
                if target_name == "publishing" {
                    return;
                }
                if target_name == "repositories" {
                    next_repos = true;
                } else if matches!(target_name, "pluginManagement" | "buildscript") {
                    next_plugin = true;
                } else if in_repos_block {
                    handle_groovy_repo_invocation(
                        target_name,
                        node,
                        source,
                        props,
                        result,
                        in_plugin_scope,
                    );
                    return;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_repos_scoped(child, source, props, result, next_repos, next_plugin);
    }
}

fn handle_groovy_repo_invocation(
    target_name: &str,
    node: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
    in_plugin_scope: bool,
) {
    match target_name {
        "google" => result.record(DeclaredRepository::Google, in_plugin_scope),
        "mavenCentral" => result.record(DeclaredRepository::MavenCentral, in_plugin_scope),
        "gradlePluginPortal" => {
            result.record(DeclaredRepository::GradlePluginPortal, in_plugin_scope);
        }
        "mavenLocal" => result.record(DeclaredRepository::MavenLocal, in_plugin_scope),
        "jcenter" => result.record(DeclaredRepository::JCenter, in_plugin_scope),
        "maven" => extract_groovy_maven_call(node, source, props, result, in_plugin_scope),
        _ => {}
    }
}

fn extract_groovy_repos_closure(
    closure: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
    in_plugin_scope: bool,
) {
    let mut cursor = closure.walk();
    let children: Vec<Node> = closure
        .children(&mut cursor)
        .filter(Node::is_named)
        .collect();
    let mut idx = 0;

    while idx < children.len() {
        let child = unwrap_expr_stmt(children[idx]);

        if child.kind() == "identifier"
            && node_text(child, source).trim() == "maven"
            && let Some(&next) = children.get(idx + 1)
            && let Some(url) = extract_split_maven_arg(next, source, props)
        {
            result.record(DeclaredRepository::MavenUrl(url), in_plugin_scope);
            idx += 2;
            continue;
        }

        if matches!(child.kind(), "method_invocation" | "command_chain")
            && let Some(target) = groovy_call_target(child)
        {
            let target_name = node_text(target, source).trim();
            handle_groovy_repo_invocation(
                target_name,
                child,
                source,
                props,
                result,
                in_plugin_scope,
            );
        }

        idx += 1;
    }
}

fn unwrap_expr_stmt(node: Node) -> Node {
    if node.kind() == "expression_statement" {
        node.child(0).unwrap_or(node)
    } else {
        node
    }
}

fn extract_split_maven_arg(
    next: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
) -> Option<String> {
    if next.kind() == "labeled_statement" {
        let label = next
            .child_by_field_name("label")
            .or_else(|| next.child(0))?;
        if node_text(label, source).trim().trim_matches(['\'', '"']) != "url" {
            return None;
        }
        let stmt = next
            .child_by_field_name("statement")
            .or_else(|| next.child(2))?;
        return extract_url_from_groovy_expr(unwrap_expr_stmt(stmt), source, props);
    }
    extract_url_from_groovy_expr(unwrap_expr_stmt(next), source, props)
}

fn extract_groovy_maven_call(
    call_or_cmd: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
    result: &mut RepositoriesResult,
    in_plugin_scope: bool,
) {
    let container = if call_or_cmd.kind() == "method_invocation" {
        let Some(arg_list) = find_child_by_kind(call_or_cmd, "argument_list") else {
            return;
        };
        arg_list
    } else {
        call_or_cmd
    };
    let skip = usize::from(call_or_cmd.kind() == "command_chain");
    let mut cursor = container.walk();
    for arg in container.children(&mut cursor).skip(skip) {
        if arg.kind() == "closure" {
            extract_groovy_maven_closure_body(arg, source, props, result, in_plugin_scope);
        } else if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
            result.record(DeclaredRepository::MavenUrl(url), in_plugin_scope);
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
        "map_literal" => {
            let mut cursor = expr.walk();
            for entry in expr.children(&mut cursor) {
                if entry.kind() == "map_entry"
                    && let Some(url) = extract_url_from_groovy_expr(entry, source, props)
                {
                    return Some(url);
                }
            }
        }
        "method_invocation" => {
            let func = groovy_call_target(expr)?;
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
            let rec = groovy_call_target(expr)?;
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
    in_plugin_scope: bool,
) {
    let mut cursor = closure.walk();
    let children: Vec<Node> = closure
        .children(&mut cursor)
        .filter(Node::is_named)
        .collect();
    let mut idx = 0;

    while idx < children.len() {
        let child = unwrap_expr_stmt(children[idx]);
        if child.kind() == "identifier"
            && node_text(child, source).trim() == "url"
            && let Some(&next) = children.get(idx + 1)
            && let Some(url) = extract_url_from_groovy_expr(unwrap_expr_stmt(next), source, props)
        {
            result.record(DeclaredRepository::MavenUrl(url), in_plugin_scope);
            idx += 2;
            continue;
        }
        if let Some(url) = extract_url_from_maven_closure_stmt(child, source, props) {
            result.record(DeclaredRepository::MavenUrl(url), in_plugin_scope);
        }
        idx += 1;
    }
}

fn extract_url_from_maven_closure_stmt(
    stmt: Node,
    source: &[u8],
    props: &AHashMap<String, String>,
) -> Option<String> {
    match stmt.kind() {
        "command_chain" => {
            let rec = groovy_call_target(stmt)?;
            if node_text(rec, source) != "url" {
                return None;
            }
            let arg = stmt
                .child_by_field_name("argument")
                .or_else(|| stmt.child(1))?;
            extract_url_from_groovy_expr(arg, source, props)
        }
        "assignment_expression" => {
            let left = stmt.child_by_field_name("left").or_else(|| stmt.child(0))?;
            if node_text(left, source).trim() != "url" {
                return None;
            }
            let right = stmt
                .child_by_field_name("right")
                .or_else(|| stmt.child(2))?;
            extract_url_from_groovy_expr(right, source, props)
        }
        "method_invocation" => {
            let func = groovy_call_target(stmt)?;
            let func_name = node_text(func, source);
            if matches!(func_name, "url" | "setUrl") {
                let arg_list = find_child_by_kind(stmt, "argument_list")?;
                let mut arg_cursor = arg_list.walk();
                for arg in arg_list.children(&mut arg_cursor) {
                    if let Some(url) = extract_url_from_groovy_expr(arg, source, props) {
                        return Some(url);
                    }
                }
            }
            None
        }
        _ => None,
    }
}
