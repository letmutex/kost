use ahash::AHashMap;
use common::concat_str;
use tree_sitter::Node;

use super::BuildScriptInfo;
use super::groovy_android::first_groovy_call_arg;
use super::kotlin::extract_catalog_accessor_node;
use super::kotlin_meta::record_plugin;
use super::util::{groovy_call_target, node_text, resolve_groovy_expr};
use crate::catalog::VersionCatalog;

pub(crate) fn extract_groovy_plugins_closure(
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
        let unwrapped = unwrap_stmt(children[idx]);

        if unwrapped.kind() == "identifier"
            && node_text(unwrapped, source).trim() == "alias"
            && idx + 1 < children.len()
        {
            resolve_and_record_plugin_alias(unwrap_stmt(children[idx + 1]), source, catalog, info);
            idx += 2;
            continue;
        }

        if matches!(unwrapped.kind(), "command_chain" | "method_invocation") {
            let consumed =
                extract_groovy_plugin_entry(&children[idx..], unwrapped, source, catalog, info);
            idx += 1 + consumed;
            continue;
        }

        idx += 1;
    }
}

fn unwrap_stmt(node: Node) -> Node {
    if node.kind() == "expression_statement" {
        node.child(0).unwrap_or(node)
    } else {
        node
    }
}

fn resolve_and_record_plugin_alias(
    arg_node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    let Some(cat) = catalog else {
        return;
    };
    let accessor = extract_catalog_accessor_node(arg_node, source)
        .unwrap_or_else(|| node_text(arg_node, source).to_string());
    if let Some((id, ver)) = cat.resolve_plugin_accessor(&accessor) {
        record_plugin(id, ver, info);
    }
}

fn extract_groovy_plugin_entry(
    remaining: &[Node],
    unwrapped: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) -> usize {
    if unwrapped.kind() == "method_invocation"
        && let Some((id, ver)) =
            extract_chained_groovy_plugin_call(unwrapped, source, &info.variables)
    {
        record_plugin(id, ver, info);
        return 0;
    }

    let mut cursor = unwrapped.walk();
    let named: Vec<Node> = unwrapped
        .children(&mut cursor)
        .filter(Node::is_named)
        .collect();
    if named.is_empty() {
        return 0;
    }

    let head = node_text(named[0], source).trim();
    if head == "alias" {
        if let Some(arg) = named
            .get(1)
            .copied()
            .or_else(|| first_groovy_call_arg(unwrapped))
        {
            resolve_and_record_plugin_alias(arg, source, catalog, info);
        }
        return 0;
    }

    let plugin_id = if unwrapped.kind() == "method_invocation" {
        extract_groovy_plugin_id_from_call(unwrapped, source, &info.variables)
    } else if named[0].kind() == "method_invocation" {
        extract_groovy_plugin_id_from_call(named[0], source, &info.variables)
    } else if matches!(head, "id" | "kotlin") && named.len() >= 2 {
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
        return 0;
    };

    if named.len() >= 3 && node_text(named[1], source).trim() == "version" {
        let ver = resolve_groovy_expr(named[2], source, &info.variables);
        record_plugin(id, ver, info);
        return 0;
    }

    let (version, consumed) =
        extract_trailing_groovy_version(&remaining[1..], source, &info.variables);
    record_plugin(id, version, info);
    consumed
}

fn extract_trailing_groovy_version(
    siblings: &[Node],
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> (Option<String>, usize) {
    let Some(&first_sib) = siblings.first() else {
        return (None, 0);
    };
    let unwrapped = unwrap_stmt(first_sib);
    if matches!(unwrapped.kind(), "command_chain" | "method_invocation")
        && let Some(target) = groovy_call_target(unwrapped)
        && node_text(target, source).trim() == "version"
    {
        let arg = if unwrapped.kind() == "method_invocation" {
            first_groovy_call_arg(unwrapped)
        } else {
            unwrapped
                .child_by_field_name("argument")
                .or_else(|| unwrapped.child(1))
        };
        let ver = arg.and_then(|node| resolve_groovy_expr(node, source, vars));
        return (ver, 1);
    }
    if unwrapped.kind() == "identifier"
        && node_text(unwrapped, source).trim() == "version"
        && let Some(&second_sib) = siblings.get(1)
    {
        let ver = resolve_groovy_expr(unwrap_stmt(second_sib), source, vars);
        return (ver, 2);
    }
    (None, 0)
}

fn extract_chained_groovy_plugin_call(
    call: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<(String, Option<String>)> {
    let func = groovy_call_target(call)?;
    if func.kind() != "field_access" {
        return None;
    }
    let obj = func
        .child_by_field_name("object")
        .or_else(|| func.child(0))?;
    let field = func
        .child_by_field_name("field")
        .or_else(|| func.child(2))?;
    let field_name = node_text(field, source).trim();
    if field_name == "apply" {
        return extract_chained_groovy_plugin_call(obj, source, vars).or_else(|| {
            let id = extract_groovy_plugin_id_from_call(obj, source, vars)?;
            Some((id, None))
        });
    }
    if field_name == "version" {
        let id = extract_groovy_plugin_id_from_call(obj, source, vars)?;
        let ver =
            first_groovy_call_arg(call).and_then(|arg| resolve_groovy_expr(arg, source, vars));
        return Some((id, ver));
    }
    None
}

fn extract_groovy_plugin_id_from_call(
    call: Node,
    source: &[u8],
    vars: &AHashMap<String, String>,
) -> Option<String> {
    let func = groovy_call_target(call)?;
    let func_name = node_text(func, source).trim();
    let arg = first_groovy_call_arg(call)?;
    let val = resolve_groovy_expr(arg, source, vars)?;
    match func_name {
        "id" => Some(val),
        "kotlin" => Some(concat_str!("org.jetbrains.kotlin.", &val)),
        _ => None,
    }
}
