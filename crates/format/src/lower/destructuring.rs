use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::delegates::parse_comment_before_init;
use crate::lower::lower_node;
use crate::lower::property::build_property_header;

pub fn lower_destructuring_declaration<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "=" => {
                docs.push(Doc::space());
                docs.push(Doc::text("="));
                docs.push(Doc::space());
            }
            ";" => {
                if !ctx.is_redundant_semicolon(&child) {
                    docs.push(Doc::text(";"));
                }
            }
            _ => {
                docs.push(lower_node(child, ctx));
            }
        }
    }
    Doc::Group(Box::new(Doc::Concat(docs)))
}

pub fn lower_backing_field<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut name_docs = Vec::new();
    let mut type_docs = Vec::new();
    let mut init_docs = Vec::new();
    let mut init_child = None;
    let mut trailing_comment = None;
    let mut in_type = false;
    let mut in_init = false;
    let mut eq_node = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ":" {
            in_type = true;
            in_init = false;
            continue;
        }
        if kind == "=" {
            eq_node = Some(child);
            in_type = false;
            in_init = true;
            continue;
        }
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if in_init && init_child.is_none() && trailing_comment.is_none() {
                trailing_comment = Some(parse_comment_before_init(child, eq_node, ctx));
            }
            continue;
        }
        if in_init {
            if init_child.is_none() {
                init_child = Some(child);
            }
            init_docs.push(lower_node(child, ctx));
        } else if in_type {
            type_docs.push(lower_node(child, ctx));
        } else if kind == "modifiers" {
            name_docs.push(lower_node(child, ctx));
            name_docs.push(Doc::space());
        } else if kind == "field" {
            name_docs.push(Doc::text("field"));
        }
    }

    let op_str = if eq_node.is_some() { Some("=") } else { None };
    build_property_header(
        name_docs,
        type_docs,
        init_docs,
        init_child,
        op_str,
        trailing_comment,
        &node,
        ctx,
    )
}
