use ahash::AHashMap;
use tree_sitter::Node;

use super::groovy_prop::extract_groovy_properties;
use super::groovy_repo::extract_groovy_repos_node;
use super::kotlin::push_extracted_coordinates;
use super::util::{
    extract_named_pair, find_child_by_kind, is_dependency_configuration, node_text,
    resolve_groovy_expr,
};
use super::{ParseError, RepositoriesResult};
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
    let source_bytes = source.as_bytes();
    let tree = GROOVY_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));
    let Some(tree) = tree else {
        return Err(ParseError::TreeSitterError(
            "Failed to parse Groovy script with tree-sitter".to_string(),
        ));
    };

    let root = tree.root_node();
    if root.has_error() {
        return Err(ParseError::SyntaxError(
            "Groovy script contains syntax errors".to_string(),
        ));
    }

    let mut active_props = properties.clone();
    extract_groovy_properties(root, source_bytes, &mut active_props);
    if let Some(deps_out) = deps {
        extract_groovy_dependencies(root, source_bytes, &active_props, deps_out);
    }
    if let Some(repos_out) = repos {
        extract_groovy_repos_node(root, source_bytes, &active_props, repos_out);
    }
    Ok(())
}

fn extract_groovy_dependencies(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    match node.kind() {
        "command_chain" => {
            if let Some(receiver) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
            {
                let rec_name = node_text(receiver, source);
                if is_dependency_configuration(rec_name)
                    || matches!(rec_name, "platform" | "enforcedPlatform")
                {
                    extract_groovy_dependency_call(node, source, properties, deps);
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
                    extract_groovy_dependency_call(node, source, properties, deps);
                    return;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_dependencies(child, source, properties, deps);
    }
}

fn extract_groovy_dependency_call(
    call_or_cmd: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    let mut named_group: Option<String> = None;
    let mut named_name: Option<String> = None;
    let mut named_ver: Option<String> = None;
    let mut positional_vals: Vec<String> = Vec::new();

    if call_or_cmd.kind() == "method_invocation" {
        if let Some(arg_list) = find_child_by_kind(call_or_cmd, "argument_list") {
            let mut cursor = arg_list.walk();
            for child in arg_list.children(&mut cursor) {
                process_groovy_dependency_arg(
                    child,
                    source,
                    properties,
                    &mut positional_vals,
                    &mut named_group,
                    &mut named_name,
                    &mut named_ver,
                    deps,
                );
            }
        }
    } else if call_or_cmd.kind() == "command_chain" {
        let mut cursor = call_or_cmd.walk();
        for child in call_or_cmd.children(&mut cursor).skip(1) {
            process_groovy_dependency_arg(
                child,
                source,
                properties,
                &mut positional_vals,
                &mut named_group,
                &mut named_name,
                &mut named_ver,
                deps,
            );
        }
    }

    push_extracted_coordinates(positional_vals, named_group, named_name, named_ver, deps);
}

#[allow(clippy::too_many_arguments)]
fn process_groovy_dependency_arg(
    arg_node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    positional_vals: &mut Vec<String>,
    named_group: &mut Option<String>,
    named_name: &mut Option<String>,
    named_ver: &mut Option<String>,
    deps: &mut Vec<DependencyCoordinate>,
) {
    match arg_node.kind() {
        "named_argument" | "map_entry" => {
            extract_named_pair(
                arg_node,
                source,
                properties,
                named_group,
                named_name,
                named_ver,
            );
        }
        "map_literal" => {
            let mut map_cursor = arg_node.walk();
            for entry in arg_node.children(&mut map_cursor) {
                if entry.kind() == "map_entry" {
                    extract_named_pair(
                        entry,
                        source,
                        properties,
                        named_group,
                        named_name,
                        named_ver,
                    );
                }
            }
        }
        "argument_list" => {
            let mut list_cursor = arg_node.walk();
            for child in arg_node.children(&mut list_cursor) {
                process_groovy_dependency_arg(
                    child,
                    source,
                    properties,
                    positional_vals,
                    named_group,
                    named_name,
                    named_ver,
                    deps,
                );
            }
        }
        "method_invocation" | "command_chain" => {
            if is_groovy_platform_call(arg_node, source) {
                extract_groovy_dependency_call(arg_node, source, properties, deps);
            }
        }
        _ if arg_node.is_named() => {
            if let Some(resolved) = resolve_groovy_expr(arg_node, source, properties) {
                positional_vals.push(resolved);
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
