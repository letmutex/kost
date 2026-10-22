use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

pub fn is_else_if_node(node: &Node<'_>) -> bool {
    if node.kind() == "if_expression" {
        return true;
    }
    if node.kind() == "control_structure_body" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "if_expression" {
                return true;
            }
        }
    }
    false
}

pub fn is_braced_body(node: &Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "block" || kind == "class_body" {
        return true;
    }
    if kind == "control_structure_body" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "{" || child.kind() == "block" {
                return true;
            }
        }
    }
    false
}

pub fn lower_jump_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut prev_is_at = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let is_at = ctx.node_text(&child).ends_with('@');
        if !docs.is_empty() && !prev_is_at {
            docs.push(Doc::space());
        }
        docs.push(lower_fn(child, ctx));
        prev_is_at = is_at;
    }
    Doc::concat(docs)
}

pub fn lower_control_structure_body<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
}

pub fn lower_type_or_range_test<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut op_text = String::new();
    let mut target = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "is" || kind == "in" {
            op_text.push_str(ctx.node_text(&child));
        } else if kind == "!" {
            op_text.push('!');
        } else {
            target = Some(lower_fn(child, ctx));
        }
    }
    let target_doc = target.unwrap_or_else(Doc::empty);
    Doc::concat(vec![Doc::smol(op_text), Doc::space(), target_doc])
}
