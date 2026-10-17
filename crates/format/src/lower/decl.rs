use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::constructors::primary_constructor_has_constructor_keyword;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;
use crate::lower::types::lower_type_constraints;

fn push_class_header_child<'a>(
    header_docs: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    prev_child_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let kind = child.kind();
    if kind == "context_receiver" {
        header_docs.push(lower_node(child, ctx));
        header_docs.push(Doc::HardLine);
        return;
    }
    if !header_docs.is_empty()
        && !matches!(
            kind,
            "," | ":" | ";" | "(" | ")" | "<" | ">" | "type_parameters" | "primary_constructor"
        )
        && !matches!(
            header_docs.last(),
            Some(Doc::Text("(") | Doc::Text("<") | Doc::Text(" ") | Doc::Text(": "))
        )
        && !header_docs
            .last()
            .is_some_and(|d| d.ends_with_line() || d.ends_with_space())
    {
        if matches!(kind, "comment" | "line_comment" | "multiline_comment")
            && prev_child_node.is_some_and(|p| ctx.has_newline_between(&p, &child))
        {
            header_docs.push(Doc::HardLine);
        } else {
            header_docs.push(Doc::space());
        }
    }
    if kind == "primary_constructor" && primary_constructor_has_constructor_keyword(&child, ctx) {
        if prev_child_node.is_some_and(|p| ctx.has_newline_between(&p, &child)) {
            header_docs.push(Doc::HardLine);
        } else {
            header_docs.push(Doc::Line);
        }
    }
    header_docs.push(lower_node(child, ctx));
}

fn build_supertypes_group<'a>(
    supertypes_docs: Vec<Doc<'a>>,
    is_empty_body: bool,
    body_doc: &mut Option<Doc<'a>>,
) -> Option<Doc<'a>> {
    if supertypes_docs.is_empty() {
        return None;
    }
    let mut inner = supertypes_docs;
    if is_empty_body && let Some(body) = body_doc.take() {
        inner.push(Doc::space());
        inner.push(body);
    }
    Some(Doc::Group(Box::new(Doc::Concat(vec![
        Doc::space(),
        Doc::text(":"),
        Doc::continuation_indent(Doc::Concat(vec![
            Doc::Line,
            Doc::Group(Box::new(Doc::Concat(inner))),
        ])),
    ]))))
}

pub fn lower_class_declaration<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut header_docs = Vec::new();
    let mut supertypes_docs = Vec::new();
    let mut where_doc = None;
    let mut where_has_newline = false;
    let mut body_doc = None;
    let mut is_after_colon = false;

    let mut cursor = node.walk();
    let mut prev_child_node: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "class_body" || kind == "enum_class_body" {
            body_doc = Some(lower_class_body(child, ctx));
        } else if kind == "type_constraints" {
            where_has_newline =
                prev_child_node.is_some_and(|p| ctx.has_newline_between(&p, &child));
            where_doc = Some(lower_type_constraints(child, ctx));
        } else if kind == ":" {
            is_after_colon = true;
        } else if is_after_colon {
            if kind == "," {
                supertypes_docs.push(Doc::text(","));
            } else {
                if !supertypes_docs.is_empty() {
                    supertypes_docs.push(Doc::Line);
                }
                supertypes_docs.push(lower_node(child, ctx));
            }
        } else {
            push_class_header_child(&mut header_docs, child, prev_child_node, ctx);
        }
        prev_child_node = Some(child);
    }

    let is_empty_body = body_doc
        .as_ref()
        .is_some_and(|b| matches!(b, Doc::Text(s) if *s == "{}"));
    let supertypes_group = build_supertypes_group(supertypes_docs, is_empty_body, &mut body_doc);

    let mut all_header = vec![Doc::Group(Box::new(Doc::Concat(header_docs)))];
    if let Some(st) = supertypes_group {
        all_header.push(st);
    }
    let where_line = if where_has_newline {
        Doc::HardLine
    } else {
        Doc::Line
    };
    if let Some(wh) = where_doc.map(|w| Doc::continuation_indent(Doc::Concat(vec![where_line, w])))
    {
        all_header.push(wh);
    }
    let header_group = Doc::Group(Box::new(Doc::Concat(all_header)));
    if let Some(body) = body_doc {
        Doc::Concat(vec![header_group, Doc::space(), body])
    } else {
        header_group
    }
}

pub use crate::lower::class_body::lower_class_body;
