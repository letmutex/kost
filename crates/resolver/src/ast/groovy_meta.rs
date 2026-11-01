use ahash::AHashMap;
use common::concat_str;
use tree_sitter::Node;

use super::BuildScriptInfo;
use super::groovy_prop::find_invocation_closure;
use super::kotlin_meta::{is_compile_sdk_ident, record_plugin};
use super::util::{find_child_by_kind, node_text, resolve_groovy_expr};
use crate::catalog::VersionCatalog;

/// Extracts `android { compileSdk }` and `plugins { ... }` metadata from Groovy DSL AST.
pub(crate) fn extract_groovy_metadata(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    if matches!(node.kind(), "command_chain" | "method_invocation")
        && let Some(target) = invocation_target(node)
    {
        let target_name = node_text(target, source).trim();
        if target_name == "android"
            && let Some(closure) = find_invocation_closure(node)
        {
            info.has_android_block = true;
            extract_groovy_android_closure(closure, source, info);
            return;
        }
        if target_name == "plugins"
            && let Some(closure) = find_invocation_closure(node)
        {
            extract_groovy_plugins_closure(closure, source, catalog, info);
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_metadata(child, source, catalog, info);
    }
}

fn invocation_target(node: Node) -> Option<Node> {
    match node.kind() {
        "command_chain" => node
            .child_by_field_name("receiver")
            .or_else(|| node.child(0)),
        "method_invocation" => node
            .child_by_field_name("function")
            .or_else(|| node.child(0)),
        _ => None,
    }
}

fn extract_groovy_android_closure(node: Node, source: &[u8], info: &mut BuildScriptInfo) {
    if info.compile_sdk.is_some() {
        return;
    }

    match node.kind() {
        "command_chain" => {
            if let Some(rec) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
                && is_compile_sdk_ident(node_text(rec, source).trim())
                && let Some(arg) = node
                    .child_by_field_name("argument")
                    .or_else(|| node.child(1))
                && let Some(sdk) = resolve_groovy_sdk_int(arg, source, &info.variables)
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
        "assignment_expression" => {
            if let Some(left) = node.child_by_field_name("left").or_else(|| node.child(0))
                && is_compile_sdk_ident(node_text(left, source).trim())
                && let Some(right) = node.child_by_field_name("right").or_else(|| node.child(2))
                && let Some(sdk) = resolve_groovy_sdk_int(right, source, &info.variables)
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
        "method_invocation" => {
            if let Some(func) = node
                .child_by_field_name("function")
                .or_else(|| node.child(0))
                && is_compile_sdk_ident(node_text(func, source).trim())
                && let Some(arg) = first_groovy_call_arg(node)
                && let Some(sdk) = resolve_groovy_sdk_int(arg, source, &info.variables)
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_android_closure(child, source, info);
    }
}

fn resolve_groovy_sdk_int(
    node: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<u32> {
    if matches!(node.kind(), "number_literal" | "integer_literal") {
        return node_text(node, source).trim().parse::<u32>().ok();
    }
    let resolved = resolve_groovy_expr(node, source, vars)?;
    resolved.trim().parse::<u32>().ok()
}

fn extract_groovy_plugins_closure(
    closure: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let mut cursor = closure.walk();
    let children: Vec<Node> = closure
        .children(&mut cursor)
        .filter(Node::is_named)
        .collect();
    let mut idx = 0;

    while idx < children.len() {
        let child = children[idx];
        let unwrapped = if child.kind() == "expression_statement" {
            child.child(0).unwrap_or(child)
        } else {
            child
        };

        if unwrapped.kind() == "identifier"
            && node_text(unwrapped, source).trim() == "alias"
            && idx + 1 < children.len()
        {
            let next_node = children[idx + 1];
            if let Some(cat) = catalog
                && let Some((id, ver)) = cat.resolve_plugin_accessor(node_text(next_node, source))
            {
                record_plugin(id, ver, info);
            }
            idx += 2;
            continue;
        }

        if unwrapped.kind() == "command_chain" {
            let next_opt = children.get(idx + 1).copied();
            if extract_groovy_plugin_command(unwrapped, next_opt, source, catalog, info) {
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }

        if unwrapped.kind() == "method_invocation" {
            let next_opt = children.get(idx + 1).copied();
            if extract_groovy_plugin_invocation(unwrapped, next_opt, source, catalog, info) {
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }

        idx += 1;
    }
}

fn extract_version_from_command(
    node: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<String> {
    let unwrapped = if node.kind() == "expression_statement" {
        node.child(0).unwrap_or(node)
    } else {
        node
    };
    if unwrapped.kind() != "command_chain" {
        return None;
    }
    let rec = unwrapped
        .child_by_field_name("receiver")
        .or_else(|| unwrapped.child(0))?;
    if node_text(rec, source).trim() != "version" {
        return None;
    }
    let arg = unwrapped
        .child_by_field_name("argument")
        .or_else(|| unwrapped.child(1))?;
    resolve_groovy_expr(arg, source, vars)
}

fn extract_groovy_plugin_command(
    cmd: Node,
    next_sibling: Option<Node>,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) -> bool {
    let mut cursor = cmd.walk();
    let named: Vec<Node> = cmd.children(&mut cursor).filter(Node::is_named).collect();
    if named.len() < 2 {
        return false;
    }

    let head = node_text(named[0], source).trim();
    if head == "alias" {
        if let Some(cat) = catalog
            && let Some((id, ver)) = cat.resolve_plugin_accessor(node_text(named[1], source))
        {
            record_plugin(id, ver, info);
        }
        return false;
    }

    let plugin_id = if named[0].kind() == "method_invocation" {
        extract_groovy_plugin_id_from_call(named[0], source, &info.variables)
    } else if matches!(head, "id" | "kotlin") {
        resolve_groovy_expr(named[1], source, &info.variables).map(|val| {
            if head == "kotlin" {
                concat_str!("org.jetbrains.kotlin.", &val)
            } else {
                val
            }
        })
    } else {
        None
    };

    let Some(id) = plugin_id else {
        return false;
    };

    let mut consumed_next = false;
    let mut version = None;
    if named.len() >= 3 && node_text(named[1], source).trim() == "version" {
        version = resolve_groovy_expr(named[2], source, &info.variables);
    } else if let Some(next_node) = next_sibling
        && let Some(ver) = extract_version_from_command(next_node, source, &info.variables)
    {
        version = Some(ver);
        consumed_next = true;
    }

    record_plugin(id, version, info);
    consumed_next
}

fn extract_groovy_plugin_invocation(
    call: Node,
    next_sibling: Option<Node>,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) -> bool {
    let Some(func) = call
        .child_by_field_name("function")
        .or_else(|| call.child(0))
    else {
        return false;
    };
    let func_name = node_text(func, source).trim();
    if func_name == "alias" {
        if let Some(arg) = first_groovy_call_arg(call)
            && let Some(cat) = catalog
            && let Some((id, ver)) = cat.resolve_plugin_accessor(node_text(arg, source))
        {
            record_plugin(id, ver, info);
        }
        return false;
    }
    let Some(id) = extract_groovy_plugin_id_from_call(call, source, &info.variables) else {
        return false;
    };
    let mut consumed_next = false;
    let mut version = None;
    if let Some(next_node) = next_sibling
        && let Some(ver) = extract_version_from_command(next_node, source, &info.variables)
    {
        version = Some(ver);
        consumed_next = true;
    }
    record_plugin(id, version, info);
    consumed_next
}

fn extract_groovy_plugin_id_from_call(
    call: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<String> {
    let func = call
        .child_by_field_name("function")
        .or_else(|| call.child(0))?;
    let func_name = node_text(func, source).trim();
    let arg = first_groovy_call_arg(call)?;
    let val = resolve_groovy_expr(arg, source, vars)?;
    match func_name {
        "id" => Some(val),
        "kotlin" => Some(concat_str!("org.jetbrains.kotlin.", &val)),
        _ => None,
    }
}

fn first_groovy_call_arg(call: Node) -> Option<Node> {
    let arg_list = find_child_by_kind(call, "argument_list")?;
    let mut cursor = arg_list.walk();
    arg_list
        .children(&mut cursor)
        .find(|child| child.is_named())
}
