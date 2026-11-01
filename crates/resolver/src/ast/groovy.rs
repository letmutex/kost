use ahash::AHashMap;
use tree_sitter::Node;

use super::groovy_dep::{extract_groovy_closure_deps, extract_groovy_dependency_call};
use super::groovy_meta::extract_groovy_metadata;
use super::groovy_prop::extract_groovy_properties;
use super::groovy_repo::extract_groovy_repos_node;
use super::groovy_sub::extract_groovy_subscript_dependency;
use super::kotlin::merge_repositories;
use super::util::{has_unrecoverable_groovy_error, is_dependency_configuration, node_text};
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

    Ok(parse_groovy_build_script_root(
        root,
        source_bytes,
        catalog,
        initial_vars,
    ))
}

pub(crate) fn parse_groovy_build_script_root(
    root: Node,
    source_bytes: &[u8],
    catalog: Option<&VersionCatalog>,
    initial_vars: &AHashMap<String, String>,
) -> BuildScriptInfo {
    let mut info = BuildScriptInfo {
        variables: initial_vars.clone(),
        ..Default::default()
    };

    extract_groovy_properties(root, source_bytes, &mut info.variables);
    extract_groovy_metadata(root, source_bytes, catalog, &mut info);
    extract_groovy_dependencies(root, source_bytes, catalog, &mut info);
    extract_groovy_repos_node(root, source_bytes, &info.variables, &mut info.repositories);

    info
}

pub(crate) fn extract_groovy_dependencies(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    match node.kind() {
        "closure" => {
            extract_groovy_closure_deps(node, source, catalog, info);
            return;
        }
        "command_chain" => {
            if let Some(receiver) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
            {
                let rec_name = node_text(receiver, source);
                if is_dependency_configuration(rec_name) {
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
        _ => {
            if extract_groovy_subscript_dependency(
                node,
                source,
                &info.variables,
                &mut info.dependencies,
            ) {
                return;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_dependencies(child, source, catalog, info);
    }
}
