use ahash::AHashMap;
use tree_sitter::Node;

use super::kotlin_meta::extract_kotlin_metadata;
use super::kotlin_prop::extract_kotlin_properties;
use super::kotlin_repo::extract_kotlin_repos_node;
use super::string::resolve_kotlin_string;
use super::util::{
    find_child_by_kind, is_dependency_configuration, node_text, resolve_kotlin_expr,
    unpack_kotlin_value_argument,
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
    target.merge(source);
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

    Ok(parse_kotlin_build_script_root(
        root,
        source_bytes,
        catalog,
        initial_vars,
    ))
}

pub(crate) fn parse_kotlin_build_script_root(
    root: Node,
    source_bytes: &[u8],
    catalog: Option<&VersionCatalog>,
    initial_vars: &AHashMap<String, String>,
) -> BuildScriptInfo {
    let mut info = BuildScriptInfo {
        variables: initial_vars.clone(),
        ..Default::default()
    };

    extract_kotlin_properties(root, source_bytes, &mut info.variables);
    extract_kotlin_metadata(root, source_bytes, catalog, &mut info);
    extract_kotlin_dependencies(root, source_bytes, catalog, &mut info);
    extract_kotlin_repos_node(root, source_bytes, &info.variables, &mut info.repositories);

    info
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
    if let Some(accessor) = extract_catalog_accessor_node(rhs, source) {
        record_catalog_accessor(&accessor, catalog, info);
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

        if val_expr.kind() == "navigation_expression"
            && let Some(accessor) = extract_catalog_accessor_node(val_expr, source)
        {
            record_catalog_accessor(&accessor, catalog, info);
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

pub(crate) fn extract_catalog_accessor_node(node: Node, source: &[u8]) -> Option<String> {
    let mut segments = Vec::new();
    collect_accessor_segments(node, source, &mut segments);
    if segments.len() >= 2 && matches!(segments[0], "libs" | "libraries") {
        Some(segments.join("."))
    } else {
        None
    }
}

fn collect_accessor_segments<'a>(node: Node<'a>, source: &'a [u8], out: &mut Vec<&'a str>) {
    match node.kind() {
        "simple_identifier" | "identifier" => {
            let text = node_text(node, source).trim();
            if !text.is_empty() {
                out.push(text);
            }
        }
        "navigation_expression" | "navigation_suffix" | "field_access" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_accessor_segments(child, source, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn record_catalog_accessor(
    accessor: &str,
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    info.catalog_accessors.push(accessor.to_string());
    if let Some(cat) = catalog {
        info.dependencies.extend(cat.resolve_accessor(accessor));
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
