use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{ListItem, format_assign_like, format_delimited_list};
use crate::lower::context::LowerContext;

pub fn lower_type_alias<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut header_parts: Vec<Doc<'a>> = Vec::new();
    let mut rhs_node = None;
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
            if rhs_node.is_none() {
                rhs_node = Some(child);
            }
            if !rhs_docs.is_empty() && !rhs_docs.last().is_some_and(|d| d.ends_with_space()) {
                rhs_docs.push(Doc::space());
            }
            rhs_docs.push(lower_fn(child, ctx));
        } else if kind == "typealias" {
            if !header_parts.is_empty() && !header_parts.last().is_some_and(|d| d.ends_with_space())
            {
                header_parts.push(Doc::space());
            }
            header_parts.push(Doc::text("typealias"));
        } else if kind == "type_parameters" {
            header_parts.push(lower_type_list(child, ctx, lower_fn));
        } else {
            if !header_parts.is_empty() && !header_parts.last().is_some_and(|d| d.ends_with_space())
            {
                header_parts.push(Doc::space());
            }
            header_parts.push(lower_fn(child, ctx));
        }
    }

    let header = Doc::concat(header_parts);
    if let Some(rhs) = rhs_node {
        let rhs_doc = Doc::concat(rhs_docs);
        format_assign_like(header, "=", rhs, rhs_doc, ctx)
    } else {
        header
    }
}

pub fn lower_type_list<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut items = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "<" || child.kind() == ">" || child.kind() == "," {
            continue;
        }
        items.push(ListItem::new(lower_fn(child, ctx)));
    }

    let has_user_trailing_comma = ctx.has_trailing_comma(&node);
    let omit_closing_break = !has_user_trailing_comma
        && ctx.options.trailing_comma_strategy == crate::options::TrailingCommaStrategy::None;
    format_delimited_list(
        "<",
        ">",
        items,
        Vec::new(),
        has_user_trailing_comma,
        ctx.options.trailing_comma_strategy,
        true,
        omit_closing_break,
        true,
    )
}

pub fn lower_type_parameter<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ":" {
            docs.push(Doc::text(" : "));
        } else {
            if !docs.is_empty()
                && !docs.last().is_some_and(|d| {
                    d.ends_with_space()
                        || d.ends_with_newline()
                        || matches!(d, Doc::Concat(v) if v.last() == Some(&Doc::Line) || v.last().is_some_and(|last| last.ends_with_space()))
                })
            {
                docs.push(Doc::space());
            }
            docs.push(lower_fn(child, ctx));
        }
    }
    Doc::concat(docs)
}

pub fn lower_type_constraints<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut items = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "where" | "," => continue,
            _ => items.push(lower_type_parameter(child, ctx, lower_fn)),
        }
    }

    if items.is_empty() {
        return Doc::empty();
    }

    let mut inner = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            inner.push(Doc::text(","));
            inner.push(Doc::Line);
        }
        inner.push(item);
    }

    Doc::concat(vec![
        Doc::text("where"),
        Doc::group(Doc::continuation_indent(Doc::concat(vec![
            Doc::Line,
            Doc::concat(inner),
        ]))),
    ])
}

pub fn lower_user_type<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "." {
            docs.push(Doc::text("."));
        } else {
            docs.push(lower_fn(child, ctx));
        }
    }
    Doc::concat(docs)
}

pub fn lower_simple_user_type<'a>(
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

pub fn lower_type_projection<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !docs.is_empty()
            && !docs
                .last()
                .is_some_and(|d: &Doc<'a>| d.ends_with_space() || d.ends_with_newline())
        {
            docs.push(Doc::space());
        }
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
}

pub fn lower_nullable_type<'a>(
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

pub fn lower_function_type<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
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
            docs.push(lower_fn(child, ctx));
        }
    }
    Doc::concat(docs)
}

pub fn lower_not_nullable_type<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "&" {
            docs.push(Doc::text(" & "));
        } else {
            docs.push(lower_fn(child, ctx));
        }
    }
    Doc::concat(docs)
}

pub fn lower_callable_reference<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut op_node = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "::" || child.kind() == "?::" {
            op_node = Some(child);
            break;
        }
    }
    let Some(op) = op_node else {
        return Doc::text(ctx.node_text(&node));
    };

    let mut receiver_docs = Vec::new();
    let mut target_doc = None;
    let mut c2 = node.walk();
    for child in node.children(&mut c2) {
        if child.end_byte() <= op.start_byte() {
            receiver_docs.push(lower_fn(child, ctx));
        } else if child.start_byte() >= op.end_byte() {
            target_doc = Some(lower_fn(child, ctx));
        }
    }

    let op_str = ctx.node_text(&op);
    let target = target_doc.unwrap_or(Doc::empty());

    if receiver_docs.is_empty() {
        return Doc::concat(vec![Doc::text(op_str), target]);
    }

    let gid = ctx.next_group_id();
    Doc::group_with_id(
        gid,
        Doc::concat(vec![
            Doc::concat(receiver_docs),
            Doc::text(op_str),
            Doc::indent_if_group_breaks(
                gid,
                crate::doc::IndentKind::Continuation,
                Doc::concat(vec![Doc::SoftLine, target]),
            ),
        ]),
    )
}

pub fn lower_delegation_specifier<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !docs.is_empty() {
            docs.push(Doc::space());
        }
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
}
