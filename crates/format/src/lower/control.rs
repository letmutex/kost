use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
pub use crate::lower::if_expr::lower_if_expression;
use crate::lower::lower_node;

pub fn lower_jump_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut prev_is_at = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let is_at = ctx.node_text(&child).ends_with('@');
        if !docs.is_empty() && !prev_is_at {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
        prev_is_at = is_at;
    }
    Doc::Concat(docs)
}

pub fn lower_control_structure_body<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut has_braces = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "{" {
            has_braces = true;
            break;
        }
    }

    if has_braces {
        crate::lower::expr::lower_block(node, ctx)
    } else {
        let mut docs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            docs.push(lower_node(child, ctx));
        }
        Doc::Concat(docs)
    }
}

pub fn lower_type_or_range_test<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
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
            target = Some(lower_node(child, ctx));
        }
    }
    let target_doc = target.unwrap_or_else(Doc::empty);
    Doc::Concat(vec![Doc::smol(op_text), Doc::space(), target_doc])
}
