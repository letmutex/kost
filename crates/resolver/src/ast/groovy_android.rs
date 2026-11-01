use ahash::AHashMap;
use tree_sitter::Node;

use super::BuildScriptInfo;
use super::kotlin_meta::is_compile_sdk_ident;
use super::util::{find_child_by_kind, groovy_call_target, node_text, resolve_groovy_expr};

pub(crate) fn extract_groovy_android_closure(
    node: Node,
    source: &[u8],
    info: &mut BuildScriptInfo,
) {
    if info.compile_sdk.is_some() {
        return;
    }

    match node.kind() {
        "closure" => {
            extract_split_android_sdk_from_closure(node, source, info);
            if info.compile_sdk.is_some() {
                return;
            }
        }
        "command_chain" => {
            if let Some(rec) = groovy_call_target(node)
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
            if let Some(func) = groovy_call_target(node)
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

fn extract_split_android_sdk_from_closure(
    closure: Node,
    source: &[u8],
    info: &mut BuildScriptInfo,
) {
    let mut cursor = closure.walk();
    let children: Vec<Node> = closure
        .children(&mut cursor)
        .filter(Node::is_named)
        .collect();
    let mut idx = 0;
    while idx + 1 < children.len() {
        let first = unwrap_stmt(children[idx]);
        if first.kind() == "identifier" && is_compile_sdk_ident(node_text(first, source).trim()) {
            let second = unwrap_stmt(children[idx + 1]);
            if let Some(sdk) = resolve_groovy_sdk_int(second, source, &info.variables) {
                info.compile_sdk = Some(sdk);
                return;
            }
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

pub(crate) fn first_groovy_call_arg(call: Node) -> Option<Node> {
    let arg_list = find_child_by_kind(call, "argument_list")?;
    let mut cursor = arg_list.walk();
    arg_list
        .children(&mut cursor)
        .find(|child| child.is_named())
}
