use ahash::AHashMap;
use common::concat_str;
use tree_sitter::Node;

use super::BuildScriptInfo;
use super::util::{
    find_child_by_kind, node_text, resolve_kotlin_expr, unpack_kotlin_value_argument,
};
use crate::catalog::VersionCatalog;

/// Extracts `android { compileSdk }` and `plugins { ... }` metadata from Kotlin DSL AST.
pub(crate) fn extract_kotlin_metadata(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    if node.kind() == "call_expression"
        && let Some(callee) = node.child(0)
    {
        let callee_name = node_text(callee, source).trim();
        if callee_name == "android" && has_lambda_block(node) {
            info.has_android_block = true;
            extract_kotlin_android_statements(node, source, info);
            return;
        }
        if callee_name == "plugins" && has_lambda_block(node) {
            extract_kotlin_plugins_block(node, source, catalog, info);
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_kotlin_metadata(child, source, catalog, info);
    }
}

fn has_lambda_block(call_node: Node) -> bool {
    find_child_by_kind(call_node, "annotated_lambda").is_some()
        || find_child_by_kind(call_node, "lambda_literal").is_some()
        || find_child_by_kind(call_node, "call_suffix").is_some_and(|suffix| {
            find_child_by_kind(suffix, "annotated_lambda").is_some()
                || find_child_by_kind(suffix, "lambda_literal").is_some()
        })
}

fn extract_kotlin_plugins_block(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    match node.kind() {
        "infix_expression" => {
            extract_kotlin_plugin_node(node, source, catalog, info);
            return;
        }
        "call_expression" => {
            if let Some(callee) = node.child(0) {
                let callee_name = node_text(callee, source).trim();
                if matches!(callee_name, "id" | "kotlin" | "alias") {
                    extract_kotlin_plugin_node(node, source, catalog, info);
                    return;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_kotlin_plugins_block(child, source, catalog, info);
    }
}

fn extract_kotlin_android_statements(node: Node, source: &[u8], info: &mut BuildScriptInfo) {
    if info.compile_sdk.is_some() {
        return;
    }

    match node.kind() {
        "assignment" => {
            if let Some(lhs) = node.child(0)
                && is_compile_sdk_ident(node_text(lhs, source).trim())
                && let Some(rhs) = node.child(2).or_else(|| node.child(1))
                && let Some(sdk) = resolve_kotlin_sdk_int(rhs, source, &info.variables)
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
        "call_expression" => {
            if let Some(callee) = node.child(0)
                && is_compile_sdk_ident(node_text(callee, source).trim())
                && let Some(arg_expr) = first_kotlin_call_arg(node, source)
                && let Some(sdk) = resolve_kotlin_sdk_int(arg_expr, source, &info.variables)
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
        "infix_expression" => {
            if let (Some(lhs), Some(rhs)) = (node.child(0), node.child(2))
                && is_compile_sdk_ident(node_text(lhs, source).trim())
                && let Some(sdk) = resolve_kotlin_sdk_int(rhs, source, &info.variables)
            {
                info.compile_sdk = Some(sdk);
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_kotlin_android_statements(child, source, info);
    }
}

pub(crate) fn is_compile_sdk_ident(name: &str) -> bool {
    name.eq_ignore_ascii_case("compileSdk") || name.eq_ignore_ascii_case("compileSdkVersion")
}

fn resolve_kotlin_sdk_int(
    node: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<u32> {
    if node.kind() == "integer_literal" {
        return node_text(node, source).trim().parse::<u32>().ok();
    }
    let resolved = resolve_kotlin_expr(node, source, vars)?;
    resolved.trim().parse::<u32>().ok()
}

fn extract_kotlin_plugin_node(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    match node.kind() {
        "infix_expression" => {
            let Some(op) = node.child(1) else {
                return;
            };
            let op_name = node_text(op, source).trim();
            if op_name == "apply" {
                if let Some(lhs) = node.child(0) {
                    extract_kotlin_plugin_node(lhs, source, catalog, info);
                }
            } else if op_name == "version"
                && let (Some(lhs), Some(rhs)) = (node.child(0), node.child(2))
                && let Some(plugin_id) = extract_plugin_call_id(lhs, source, &info.variables)
            {
                let version = resolve_kotlin_expr(rhs, source, &info.variables);
                record_plugin(plugin_id, version, info);
            }
        }
        "call_expression" => {
            if let Some(callee) = node.child(0) {
                let callee_text = node_text(callee, source).trim();
                if callee_text == "alias" {
                    if let Some(arg_expr) = first_kotlin_call_arg(node, source)
                        && let Some(cat) = catalog
                        && let Some((id, ver)) =
                            cat.resolve_plugin_accessor(node_text(arg_expr, source))
                    {
                        record_plugin(id, ver, info);
                    }
                    return;
                }
                if let Some(plugin_id) = extract_plugin_call_id(node, source, &info.variables) {
                    record_plugin(plugin_id, None, info);
                }
            }
        }
        _ => {}
    }
}

fn extract_plugin_call_id(
    call_node: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<String> {
    if call_node.kind() != "call_expression" {
        return None;
    }
    let callee = call_node.child(0)?;
    let callee_name = node_text(callee, source).trim();
    let arg_expr = first_kotlin_call_arg(call_node, source)?;
    let arg_val = resolve_kotlin_expr(arg_expr, source, vars)?;
    match callee_name {
        "id" => Some(arg_val),
        "kotlin" => Some(concat_str!("org.jetbrains.kotlin.", &arg_val)),
        _ => None,
    }
}

fn first_kotlin_call_arg<'a>(call_node: Node<'a>, source: &'a [u8]) -> Option<Node<'a>> {
    let suffix = find_child_by_kind(call_node, "call_suffix")?;
    let val_args = find_child_by_kind(suffix, "value_arguments")?;
    let mut cursor = val_args.walk();
    for arg in val_args.children(&mut cursor) {
        if arg.kind() == "value_argument"
            && let Some((_, expr)) = unpack_kotlin_value_argument(arg, source)
        {
            return Some(expr);
        }
    }
    None
}

pub(crate) fn record_plugin(id: String, version: Option<String>, info: &mut BuildScriptInfo) {
    if id.contains("kotlin")
        && let Some(ref ver) = version
    {
        info.kotlin_versions.push(ver.clone());
    }
    info.plugins.push((id, version));
}
