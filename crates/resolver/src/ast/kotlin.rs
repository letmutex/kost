use ahash::AHashMap;
use tree_sitter::Node;

use super::kotlin_meta::extract_kotlin_metadata;
use super::kotlin_prop::extract_kotlin_properties;
use super::kotlin_repo::extract_kotlin_repos_node;
use super::util::{
    find_child_by_kind, is_catalog_accessor, is_dependency_configuration, node_text,
    resolve_kotlin_expr, resolve_kotlin_string, unpack_kotlin_value_argument,
};
use super::{BuildScriptInfo, ParseError, RepositoriesResult};
use crate::catalog::VersionCatalog;
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
    let info = parse_kotlin_build_script(source, None, properties, true)?;
    if let Some(deps_out) = deps {
        deps_out.extend(info.dependencies);
    }
    if let Some(repos_out) = repos {
        merge_repositories(repos_out, info.repositories);
    }
    Ok(())
}

pub(crate) fn merge_repositories(target: &mut RepositoriesResult, source: RepositoriesResult) {
    target.urls.extend(source.urls);
    target.google |= source.google;
    target.maven_central |= source.maven_central;
    target.gradle_plugin_portal |= source.gradle_plugin_portal;
    target.maven_local |= source.maven_local;
}

/// Parse full `BuildScriptInfo` from a Kotlin `.gradle.kts` script.
pub fn parse_kotlin_build_script(
    source: &str,
    catalog: Option<&VersionCatalog>,
    initial_vars: &AHashMap<String, String>,
    strict: bool,
) -> Result<BuildScriptInfo, ParseError> {
    let source_bytes = source.as_bytes();
    let tree = KOTLIN_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));
    let Some(tree) = tree else {
        return Err(ParseError::TreeSitterError(
            "Failed to parse Kotlin script with tree-sitter".to_string(),
        ));
    };

    let root = tree.root_node();
    if strict && root.has_error() {
        return Err(ParseError::SyntaxError(
            "Kotlin script contains syntax errors".to_string(),
        ));
    }

    let mut info = BuildScriptInfo {
        variables: initial_vars.clone(),
        ..Default::default()
    };

    extract_kotlin_properties(root, source_bytes, &mut info.variables);
    extract_kotlin_metadata(root, source_bytes, catalog, &mut info);
    extract_kotlin_dependencies(root, source_bytes, catalog, &mut info);
    extract_kotlin_repos_node(root, source_bytes, &info.variables, &mut info.repositories);

    Ok(info)
}

fn extract_kotlin_dependencies(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    if node.kind() == "call_expression" {
        if let Some(callee) = node.child(0) {
            let callee_name = node_text(callee, source).trim_matches('"');
            if is_dependency_configuration(callee_name) {
                extract_call_dependency(node, source, catalog, info);
                return;
            }
        }
    } else if node.kind() == "infix_expression"
        && let Some(operator_node) = node.child(1)
    {
        let op_name = node_text(operator_node, source);
        if is_dependency_configuration(op_name) {
            extract_infix_dependency(node, source, catalog, info);
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_kotlin_dependencies(child, source, catalog, info);
    }
}

fn extract_infix_dependency(
    infix_node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let Some(rhs) = infix_node.child(2) else {
        return;
    };
    let rhs_text = node_text(rhs, source).trim();
    if is_catalog_accessor(rhs_text) {
        record_catalog_accessor(rhs_text, catalog, info);
        return;
    }
    if let Some(raw_coord) = resolve_kotlin_expr(rhs, source, &info.variables).or_else(|| {
        (rhs.kind() == "string_literal")
            .then(|| resolve_kotlin_string(rhs, source, &info.variables))
    }) && let Some(coord) = DependencyCoordinate::parse(&raw_coord)
    {
        info.dependencies.push(coord);
    }
}

fn extract_call_dependency(
    call_node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
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
            if let Some(val) = resolve_kotlin_expr(val_expr, source, &info.variables) {
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
                extract_call_dependency(val_expr, source, catalog, info);
            }
            continue;
        }

        let expr_text = node_text(val_expr, source).trim();
        if val_expr.kind() == "navigation_expression" && is_catalog_accessor(expr_text) {
            record_catalog_accessor(expr_text, catalog, info);
            continue;
        }

        if let Some(val) = resolve_kotlin_expr(val_expr, source, &info.variables) {
            positional_vals.push(val);
        }
    }

    push_extracted_coordinates(
        positional_vals,
        named_group,
        named_name,
        named_ver,
        &mut info.dependencies,
    );
}

pub(crate) fn record_catalog_accessor(
    raw_accessor: &str,
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let clean: String = raw_accessor
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    info.catalog_accessors.push(clean.clone());
    if let Some(cat) = catalog {
        info.dependencies.extend(cat.resolve_accessor(&clean));
    }
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
