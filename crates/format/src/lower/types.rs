use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lists::{
    CommaListOptions, extract_comma_list_items, format_comma_list, has_newline_after_first_token,
};
use crate::lower::lower_node;

pub fn lower_type_alias<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut header_parts: Vec<Doc<'a>> = Vec::new();
    let mut rhs_docs: Vec<Doc<'a>> = Vec::new();
    let mut is_after_equals = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" {
            is_after_equals = true;
            continue;
        }

        if is_after_equals {
            if !rhs_docs.is_empty()
                && !rhs_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                rhs_docs.push(Doc::space());
            }
            rhs_docs.push(lower_node(child, ctx));
        } else if kind == "typealias" {
            if !header_parts.is_empty()
                && !header_parts
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                header_parts.push(Doc::space());
            }
            header_parts.push(Doc::text("typealias "));
        } else if kind == "type_parameters" {
            header_parts.push(lower_type_list(child, ctx));
        } else {
            if !header_parts.is_empty()
                && !header_parts
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                header_parts.push(Doc::space());
            }
            header_parts.push(lower_node(child, ctx));
        }
    }

    let header = Doc::Concat(header_parts);
    if rhs_docs.is_empty() {
        return header;
    }
    let rhs_doc = Doc::Concat(rhs_docs);

    Doc::Group(Box::new(Doc::Concat(vec![
        header,
        Doc::space(),
        Doc::text("="),
        Doc::continuation_indent(Doc::Concat(vec![Doc::Line, rhs_doc])),
    ])))
}

pub fn lower_type_list<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let has_user_newline = ctx.node_text(&node).contains('\n');
    let has_break_after_open = has_newline_after_first_token(&node, ctx);
    let (items, has_trailing_comma) = extract_comma_list_items(node, ctx);
    format_comma_list(
        items,
        has_trailing_comma,
        "<",
        ">",
        CommaListOptions {
            strategy: ctx.options.trailing_comma_strategy,
            is_declaration: false,
            has_user_newline,
            has_break_after_open,
            is_single_unnamed_lambda: false,
            is_kotlinlang: ctx.options.block_indent == 4,
        },
    )
}

pub fn lower_type_parameter<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ":" {
            docs.push(Doc::text(" : "));
        } else {
            if !docs.is_empty()
                && !matches!(docs.last(), Some(Doc::Text(" : ")))
                && !docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                docs.push(Doc::space());
            }
            docs.push(lower_node(child, ctx));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_type_constraints<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let has_user_newline = ctx.node_text(&node).contains('\n');
    let mut items = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "where" | "," => continue,
            _ => items.push(lower_type_parameter(child, ctx)),
        }
    }

    if items.is_empty() {
        return Doc::empty();
    }

    let sep_line = if has_user_newline {
        Doc::HardLine
    } else {
        Doc::Line
    };

    let mut inner = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            inner.push(Doc::text(","));
            inner.push(sep_line.clone());
        }
        inner.push(item);
    }

    Doc::Group(Box::new(Doc::Concat(vec![
        Doc::text("where"),
        Doc::continuation_indent(Doc::Concat(vec![sep_line, Doc::Concat(inner)])),
    ])))
}

pub fn lower_function_type<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "->" {
            docs.push(Doc::text(" -> "));
        } else {
            if child.kind() == "receiver_type" && !docs.is_empty() {
                docs.push(Doc::space());
            }
            docs.push(lower_node(child, ctx));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_delegation_specifier<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !docs.is_empty() {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
    }
    Doc::Concat(docs)
}

pub fn lower_enum_entry<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body" {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
    }
    Doc::Concat(docs)
}
