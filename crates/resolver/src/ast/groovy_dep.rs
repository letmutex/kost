use ahash::AHashMap;
use tree_sitter::Node;

use super::BuildScriptInfo;
use super::groovy_sub::extract_groovy_subscript_dependency;
use super::kotlin::{
    extract_catalog_accessor_node, push_extracted_coordinates, record_catalog_accessor,
};
use super::util::{
    extract_labeled_pair, extract_named_pair, find_child_by_kind, is_dependency_configuration,
    is_groovy_platform_call, node_text, resolve_groovy_expr,
};
use crate::catalog::VersionCatalog;
use crate::coordinate::DependencyCoordinate;

#[derive(Default)]
pub(crate) struct GroovyDepCallArgs {
    pub positional_vals: Vec<String>,
    pub named_group: Option<String>,
    pub named_name: Option<String>,
    pub named_ver: Option<String>,
}

pub(crate) fn extract_groovy_closure_deps(
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

        if extract_groovy_subscript_dependency(
            child,
            source,
            &info.variables,
            &mut info.dependencies,
        ) {
            idx += 1;
            continue;
        }

        if child.kind() == "command_chain" {
            let next_opt = children.get(idx + 1).copied();
            if try_extract_concatenated_command_chain(
                child,
                next_opt,
                source,
                &info.variables,
                &mut info.dependencies,
            ) {
                idx += 2;
                continue;
            }
            if let Some(receiver) = child
                .child_by_field_name("receiver")
                .or_else(|| child.child(0))
                && is_dependency_configuration(node_text(receiver, source).trim())
            {
                extract_groovy_dependency_call(child, source, catalog, info);
                idx += 1;
                continue;
            }
        }

        if child.kind() == "method_invocation"
            && let Some(function) = child
                .child_by_field_name("function")
                .or_else(|| child.child(0))
            && is_dependency_configuration(node_text(function, source).trim())
        {
            extract_groovy_dependency_call(child, source, catalog, info);
            idx += 1;
            continue;
        }

        if is_split_config_stmt(child, source) {
            idx += 1;
            idx = handle_split_closure_config_stmt(&children, idx, source, catalog, info);
            continue;
        }

        super::groovy::extract_groovy_dependencies(child, source, catalog, info);
        idx += 1;
    }
}

fn is_split_config_stmt(child: Node, source: &[u8]) -> bool {
    child.kind() == "expression_statement"
        && child.child(0).is_some_and(|ident| {
            ident.kind() == "identifier"
                && is_dependency_configuration(node_text(ident, source).trim())
        })
}

fn handle_split_closure_config_stmt(
    children: &[Node],
    mut idx: usize,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) -> usize {
    if let Some(&next) = children.get(idx)
        && try_resolve_split_dep_arg(next, source, catalog, info)
    {
        return idx + 1;
    }

    let mut args = GroovyDepCallArgs::default();
    while idx < children.len() {
        let next = children[idx];
        if next.kind() == "labeled_statement" {
            extract_labeled_pair(
                next,
                source,
                &info.variables,
                &mut args.named_group,
                &mut args.named_name,
                &mut args.named_ver,
            );
            idx += 1;
        } else if (next.kind() == "ERROR" && node_text(next, source).trim() == ",")
            || next.kind().contains("comment")
        {
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
    idx
}

pub(crate) fn try_extract_concatenated_command_chain(
    cmd_node: Node,
    next_node: Option<Node>,
    source: &[u8],
    vars: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
) -> bool {
    let Some(receiver) = cmd_node
        .child_by_field_name("receiver")
        .or_else(|| cmd_node.child(0))
    else {
        return false;
    };
    if !is_dependency_configuration(node_text(receiver, source).trim()) {
        return false;
    }
    let Some(next) = next_node else {
        return false;
    };
    let inner = if next.kind() == "expression_statement" {
        next.child(0).unwrap_or(next)
    } else {
        next
    };
    if inner.kind() != "unary_expression" {
        return false;
    }
    let Some(op) = inner.child(0) else {
        return false;
    };
    if node_text(op, source).trim() != "+" {
        return false;
    }
    let Some(operand) = inner
        .child_by_field_name("operand")
        .or_else(|| inner.child(1))
    else {
        return false;
    };
    let Some(arg) = cmd_node
        .child_by_field_name("argument")
        .or_else(|| cmd_node.child(1))
    else {
        return false;
    };
    let Some(first_str) = resolve_groovy_expr(arg, source, vars) else {
        return false;
    };
    let Some(second_str) = resolve_groovy_expr(operand, source, vars) else {
        return false;
    };
    let full = common::concat_str!(&first_str, &second_str);
    if let Some(coord) = DependencyCoordinate::parse(&full) {
        deps.push(coord);
        return true;
    }
    false
}

pub(crate) fn try_resolve_split_dep_arg(
    next: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) -> bool {
    if matches!(next.kind(), "labeled_statement" | "ERROR") || next.kind().contains("comment") {
        return false;
    }
    let inner = if next.kind() == "expression_statement" {
        next.child(0).unwrap_or(next)
    } else {
        next
    };
    if inner.kind() == "identifier" && is_dependency_configuration(node_text(inner, source).trim())
    {
        return false;
    }
    let before_deps = info.dependencies.len();
    let before_accessors = info.catalog_accessors.len();
    let mut args = GroovyDepCallArgs::default();
    process_groovy_dependency_arg(inner, source, catalog, &mut args, info);
    push_extracted_coordinates(
        args.positional_vals,
        args.named_group,
        args.named_name,
        args.named_ver,
        &mut info.dependencies,
    );
    info.dependencies.len() > before_deps || info.catalog_accessors.len() > before_accessors
}

pub(crate) fn extract_groovy_dependency_call(
    call_or_cmd: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let mut args = GroovyDepCallArgs::default();
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
    for child in container.children(&mut cursor).skip(skip) {
        process_groovy_dependency_arg(child, source, catalog, &mut args, info);
    }

    push_extracted_coordinates(
        args.positional_vals,
        args.named_group,
        args.named_name,
        args.named_ver,
        &mut info.dependencies,
    );
}

pub(crate) fn process_groovy_dependency_arg(
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
        "field_access" => {
            if let Some(accessor) = extract_catalog_accessor_node(arg_node, source) {
                record_catalog_accessor(&accessor, catalog, info);
            } else if let Some(resolved) = resolve_groovy_expr(arg_node, source, &info.variables) {
                args.positional_vals.push(resolved);
            }
        }
        _ if arg_node.is_named() => {
            if let Some(resolved) = resolve_groovy_expr(arg_node, source, &info.variables) {
                args.positional_vals.push(resolved);
            }
        }
        _ => {}
    }
}
