use ahash::AHashMap;
use tree_sitter::Node;

use super::groovy_meta::extract_groovy_metadata;
use super::groovy_prop::extract_groovy_properties;
use super::groovy_repo::extract_groovy_repos_node;
use super::kotlin::{merge_repositories, push_extracted_coordinates, record_catalog_accessor};
use super::util::{
    extract_named_pair, find_child_by_kind, has_unrecoverable_groovy_error, is_catalog_accessor,
    is_dependency_configuration, node_text, resolve_groovy_expr,
};
use super::{BuildScriptInfo, ParseError, RepositoriesResult};
use crate::catalog::VersionCatalog;
use crate::coordinate::DependencyCoordinate;

thread_local! {
    pub(crate) static GROOVY_PARSER: std::cell::RefCell<tree_sitter::Parser> = std::cell::RefCell::new({
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language =
            unsafe { std::mem::transmute((dekobon_tree_sitter_groovy::LANGUAGE.into_raw())()) };
        parser
            .set_language(&language)
            .expect("Failed to load Groovy tree-sitter grammar");
        parser
    });
}

/// Parse dependencies and property declarations from Groovy script content.
pub fn parse_groovy_script(
    source: &str,
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) -> Result<(), ParseError> {
    parse_groovy_script_with_repos(source, properties, Some(deps), None)
}

/// Parse Groovy script once, extracting properties, optional dependencies, and optional repositories.
pub(crate) fn parse_groovy_script_with_repos(
    source: &str,
    properties: &AHashMap<String, String>,
    deps: Option<&mut Vec<DependencyCoordinate>>,
    repos: Option<&mut RepositoriesResult>,
) -> Result<(), ParseError> {
    let info = parse_groovy_build_script(source, None, properties, true)?;
    if let Some(deps_out) = deps {
        deps_out.extend(info.dependencies);
    }
    if let Some(repos_out) = repos {
        merge_repositories(repos_out, info.repositories);
    }
    Ok(())
}

/// Parse full `BuildScriptInfo` from a Groovy `.gradle` script.
pub fn parse_groovy_build_script(
    source: &str,
    catalog: Option<&VersionCatalog>,
    initial_vars: &AHashMap<String, String>,
    strict: bool,
) -> Result<BuildScriptInfo, ParseError> {
    let source_bytes = source.as_bytes();
    let tree = GROOVY_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));
    let Some(tree) = tree else {
        return Err(ParseError::TreeSitterError(
            "Failed to parse Groovy script with tree-sitter".to_string(),
        ));
    };

    let root = tree.root_node();
    if strict && has_unrecoverable_groovy_error(root, source_bytes) {
        return Err(ParseError::SyntaxError(
            "Groovy script contains syntax errors".to_string(),
        ));
    }

    let mut info = BuildScriptInfo {
        variables: initial_vars.clone(),
        ..Default::default()
    };

    extract_groovy_properties(root, source_bytes, &mut info.variables);
    extract_groovy_metadata(root, source_bytes, catalog, &mut info);
    extract_groovy_dependencies(root, source_bytes, catalog, &mut info);
    extract_groovy_repos_node(root, source_bytes, &info.variables, &mut info.repositories);

    Ok(info)
}

fn extract_groovy_dependencies(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    match node.kind() {
        "closure" => {
            extract_groovy_closure_split_deps(node, source, catalog, info);
        }
        "command_chain" => {
            if let Some(receiver) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
            {
                let rec_name = node_text(receiver, source);
                if is_dependency_configuration(rec_name)
                    || matches!(rec_name, "platform" | "enforcedPlatform")
                {
                    extract_groovy_dependency_call(node, source, catalog, info);
                    return;
                }
            }
        }
        "method_invocation" => {
            if let Some(function) = node
                .child_by_field_name("function")
                .or_else(|| node.child(0))
            {
                let func_name = node_text(function, source);
                if is_dependency_configuration(func_name) {
                    extract_groovy_dependency_call(node, source, catalog, info);
                    return;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_dependencies(child, source, catalog, info);
    }
}

fn extract_groovy_closure_split_deps(
    closure: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let mut cursor = closure.walk();
    let children: Vec<Node> = closure.children(&mut cursor).collect();
    let mut idx = 0;

    while idx < children.len() {
        let child = children[idx];
        let is_config_stmt = child.kind() == "expression_statement"
            && child
                .child(0)
                .is_some_and(|ident| is_dependency_configuration(node_text(ident, source).trim()));

        if !is_config_stmt {
            idx += 1;
            continue;
        }

        idx += 1;
        if let Some(&next) = children.get(idx) {
            let inner = if next.kind() == "expression_statement" {
                next.child(0).unwrap_or(next)
            } else {
                next
            };
            let text = node_text(inner, source).trim();
            if inner.kind() == "field_access" {
                if is_catalog_accessor(text) {
                    record_catalog_accessor(text, catalog, info);
                    idx += 1;
                    continue;
                }
                if let Some(resolved) = resolve_groovy_expr(inner, source, &info.variables)
                    && let Some(coord) = DependencyCoordinate::parse(&resolved)
                {
                    info.dependencies.push(coord);
                    idx += 1;
                    continue;
                }
            }
        }

        let mut args = GroovyDepCallArgs::default();
        while idx < children.len() {
            let next = children[idx];
            if next.kind() == "labeled_statement" {
                extract_labeled_pair(next, source, &info.variables, &mut args);
                idx += 1;
            } else if next.kind() == "ERROR" && node_text(next, source).trim() == "," {
                idx += 1;
            } else {
                break;
            }
        }

        push_extracted_coordinates(
            args.positional_vals,
            args.named_group,
            args.named_name,
            args.named_ver,
            &mut info.dependencies,
        );
    }
}

fn extract_labeled_pair(
    labeled: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
    args: &mut GroovyDepCallArgs,
) {
    let Some(label_node) = labeled
        .child_by_field_name("label")
        .or_else(|| labeled.child(0))
    else {
        return;
    };
    let Some(stmt_node) = labeled
        .child_by_field_name("statement")
        .or_else(|| labeled.child(2))
    else {
        return;
    };
    let val_node = if stmt_node.kind() == "expression_statement" {
        stmt_node.child(0).unwrap_or(stmt_node)
    } else {
        stmt_node
    };
    let key = node_text(label_node, source)
        .trim()
        .trim_matches(['\'', '"']);
    if let Some(val) = resolve_groovy_expr(val_node, source, vars) {
        match key {
            "group" => args.named_group = Some(val),
            "name" => args.named_name = Some(val),
            "version" => args.named_ver = Some(val),
            _ => {}
        }
    }
}

#[derive(Default)]
struct GroovyDepCallArgs {
    positional_vals: Vec<String>,
    named_group: Option<String>,
    named_name: Option<String>,
    named_ver: Option<String>,
}

fn extract_groovy_dependency_call(
    call_or_cmd: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let mut args = GroovyDepCallArgs::default();

    if call_or_cmd.kind() == "method_invocation" {
        if let Some(arg_list) = find_child_by_kind(call_or_cmd, "argument_list") {
            let mut cursor = arg_list.walk();
            for child in arg_list.children(&mut cursor) {
                process_groovy_dependency_arg(child, source, catalog, &mut args, info);
            }
        }
    } else if call_or_cmd.kind() == "command_chain" {
        let mut cursor = call_or_cmd.walk();
        for child in call_or_cmd.children(&mut cursor).skip(1) {
            process_groovy_dependency_arg(child, source, catalog, &mut args, info);
        }
    }

    push_extracted_coordinates(
        args.positional_vals,
        args.named_group,
        args.named_name,
        args.named_ver,
        &mut info.dependencies,
    );
}

fn process_groovy_dependency_arg(
    arg_node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    args: &mut GroovyDepCallArgs,
    info: &mut BuildScriptInfo,
) {
    match arg_node.kind() {
        "named_argument" | "map_entry" => {
            extract_named_pair(
                arg_node,
                source,
                &info.variables,
                &mut args.named_group,
                &mut args.named_name,
                &mut args.named_ver,
            );
        }
        "map_literal" => {
            let mut map_cursor = arg_node.walk();
            for entry in arg_node.children(&mut map_cursor) {
                if entry.kind() == "map_entry" {
                    extract_named_pair(
                        entry,
                        source,
                        &info.variables,
                        &mut args.named_group,
                        &mut args.named_name,
                        &mut args.named_ver,
                    );
                }
            }
        }
        "argument_list" => {
            let mut list_cursor = arg_node.walk();
            for child in arg_node.children(&mut list_cursor) {
                process_groovy_dependency_arg(child, source, catalog, args, info);
            }
        }
        "method_invocation" | "command_chain" => {
            if is_groovy_platform_call(arg_node, source) {
                extract_groovy_dependency_call(arg_node, source, catalog, info);
            }
        }
        "field_access" if is_catalog_accessor(node_text(arg_node, source)) => {
            record_catalog_accessor(node_text(arg_node, source), catalog, info);
        }
        _ if arg_node.is_named() => {
            if let Some(resolved) = resolve_groovy_expr(arg_node, source, &info.variables) {
                args.positional_vals.push(resolved);
            }
        }
        _ => {}
    }
}

fn is_groovy_platform_call(node: Node, source: &[u8]) -> bool {
    let target = match node.kind() {
        "method_invocation" => node
            .child_by_field_name("function")
            .or_else(|| node.child(0)),
        "command_chain" => node
            .child_by_field_name("receiver")
            .or_else(|| node.child(0)),
        _ => None,
    };
    target.is_some_and(|target_node| {
        matches!(
            node_text(target_node, source),
            "platform" | "enforcedPlatform"
        )
    })
}
