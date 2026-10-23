use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::class_body::lower_class_body;
use crate::lower::constructors::primary_constructor_has_constructor_keyword;
use crate::lower::context::LowerContext;

#[derive(Default)]
struct ClassDeclParts<'a> {
    modifier_docs: Vec<Doc<'a>>,
    header_docs: Vec<Doc<'a>>,
    supertypes_docs: Vec<Doc<'a>>,
    where_doc: Option<Doc<'a>>,
    body_doc: Option<Doc<'a>>,
}

fn push_class_header_comment<'a>(
    parts: &mut ClassDeclParts<'a>,
    child: Node<'a>,
    prev_child_node: Option<Node<'a>>,
    has_seen_primary_constructor: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    if crate::rules::comments::try_merge_continuation_comment(
        prev_child_node,
        &child,
        &mut parts.header_docs,
        ctx,
    ) || crate::rules::comments::try_merge_continuation_comment(
        prev_child_node,
        &child,
        &mut parts.modifier_docs,
        ctx,
    ) {
        return;
    }
    let header_docs = &mut parts.header_docs;
    let is_same_line = prev_child_node.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
    if is_same_line {
        let prev_is_modifiers = prev_child_node.is_some_and(|p| p.kind() == "modifiers");
        if !prev_is_modifiers && !header_docs.last().is_some_and(|d| d.ends_with_space()) {
            header_docs.push(Doc::space());
        }
        let child_doc = if child.kind() == "line_comment" {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        header_docs.push(child_doc);
        header_docs.push(Doc::HardLine);
    } else if has_seen_primary_constructor {
        header_docs.push(Doc::indent(Doc::concat(vec![
            Doc::HardLine,
            lower_fn(child, ctx),
        ])));
        header_docs.push(Doc::HardLine);
    } else {
        if !header_docs.last().is_some_and(|d| d.ends_with_newline()) {
            header_docs.push(Doc::HardLine);
        }
        header_docs.push(lower_fn(child, ctx));
        header_docs.push(Doc::HardLine);
    }
}

fn handle_class_in_colon<'a>(
    child: Node<'a>,
    prev_child_node: Option<Node<'a>>,
    parts: &mut ClassDeclParts<'a>,
    in_colon: &mut bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    match child.kind() {
        "type_constraints" => {
            *in_colon = false;
            parts.where_doc = Some(lower_fn(child, ctx));
        }
        "class_body" | "enum_class_body" => {
            *in_colon = false;
            parts.body_doc = Some(lower_class_body(child, ctx, lower_fn));
        }
        "comment" | "line_comment" | "multiline_comment" => {
            let is_same_line =
                prev_child_node.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
            if is_same_line {
                parts.supertypes_docs.push(Doc::space());
            } else {
                parts.supertypes_docs.push(Doc::HardLine);
            }
            parts.supertypes_docs.push(lower_fn(child, ctx));
        }
        "annotation" => {
            if !parts.supertypes_docs.is_empty() {
                parts.supertypes_docs.push(Doc::Line);
            }
            parts.supertypes_docs.push(lower_fn(child, ctx));
            parts.supertypes_docs.push(Doc::space());
        }
        "," => {
            parts.supertypes_docs.push(Doc::text(","));
        }
        _ => {
            if !parts.supertypes_docs.is_empty() {
                if prev_child_node.is_some_and(|p| ctx.has_blank_line_between(&p, &child)) {
                    parts.supertypes_docs.push(Doc::EmptyLine);
                } else if !parts
                    .supertypes_docs
                    .last()
                    .is_some_and(|d| d.ends_with_newline() || d.ends_with_space())
                {
                    parts.supertypes_docs.push(Doc::Line);
                }
            }
            parts.supertypes_docs.push(lower_fn(child, ctx));
        }
    }
}

fn push_class_primary_constructor<'a>(
    parts: &mut ClassDeclParts<'a>,
    child: Node<'a>,
    prev_child_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    if primary_constructor_has_constructor_keyword(&child, ctx) {
        if prev_child_node.is_some_and(|p| ctx.has_user_newline_between(&p, &child)) {
            parts.header_docs.push(Doc::HardLine);
        } else {
            parts.header_docs.push(Doc::Line);
        }
    }
    parts.header_docs.push(lower_fn(child, ctx));
}

fn handle_class_header_child<'a>(
    child: Node<'a>,
    prev_child_node: Option<Node<'a>>,
    parts: &mut ClassDeclParts<'a>,
    in_colon: &mut bool,
    has_seen_primary_constructor: &mut bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    match kind {
        "modifiers" | "annotation" => parts.modifier_docs.push(lower_fn(child, ctx)),
        "context_receiver" | "context_parameters" => {
            parts.modifier_docs.push(lower_fn(child, ctx));
            parts.modifier_docs.push(Doc::HardLine);
        }
        "class" | "interface" | "object" | "enum" => {
            if !parts.header_docs.is_empty()
                && !parts
                    .header_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
            {
                parts.header_docs.push(Doc::space());
            }
            parts.header_docs.push(Doc::text(ctx.node_text(&child)));
        }
        "companion" => parts.header_docs.push(Doc::text("companion")),
        ";" => {
            if !ctx.is_redundant_semicolon(&child) {
                parts.header_docs.push(Doc::text(";"));
            }
        }
        ":" => *in_colon = true,
        "type_constraints" => {
            parts.where_doc = Some(lower_fn(child, ctx));
        }
        "class_body" | "enum_class_body" => {
            parts.body_doc = Some(lower_class_body(child, ctx, lower_fn));
        }
        "comment" | "line_comment" | "multiline_comment" => {
            push_class_header_comment(
                parts,
                child,
                prev_child_node,
                *has_seen_primary_constructor,
                ctx,
                lower_fn,
            );
        }
        "primary_constructor" => {
            push_class_primary_constructor(parts, child, prev_child_node, ctx, lower_fn);
            *has_seen_primary_constructor = true;
        }
        "type_parameters" => {
            parts.header_docs.push(lower_fn(child, ctx));
        }
        _ => {
            if !parts.header_docs.is_empty()
                && !parts
                    .header_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
            {
                parts.header_docs.push(Doc::space());
            }
            parts.header_docs.push(lower_fn(child, ctx));
        }
    }
}

fn parse_class_parts<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> ClassDeclParts<'a> {
    let mut parts = ClassDeclParts::default();
    let mut in_colon = false;
    let mut prev_child_node: Option<Node<'a>> = None;
    let mut has_seen_primary_constructor = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if in_colon {
            handle_class_in_colon(
                child,
                prev_child_node,
                &mut parts,
                &mut in_colon,
                ctx,
                lower_fn,
            );
        } else {
            handle_class_header_child(
                child,
                prev_child_node,
                &mut parts,
                &mut in_colon,
                &mut has_seen_primary_constructor,
                ctx,
                lower_fn,
            );
        }
        prev_child_node = Some(child);
    }
    parts
}

pub fn lower_class_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut parts = parse_class_parts(node, ctx, lower_fn);

    let mut full_header = Vec::new();
    if !parts.modifier_docs.is_empty() {
        full_header.extend(parts.modifier_docs);
    }
    full_header.extend(parts.header_docs);

    if node.kind() == "companion_object"
        && parts.body_doc.is_none()
        && parts.supertypes_docs.is_empty()
        && node.next_named_sibling().is_some()
    {
        full_header.push(Doc::text(";"));
    }

    let is_empty_body = parts
        .body_doc
        .as_ref()
        .is_some_and(|b| matches!(b, Doc::Text(s) if *s == "{}"));

    let supertypes_group = if !parts.supertypes_docs.is_empty() {
        let mut inner = parts.supertypes_docs;
        if is_empty_body && let Some(body) = parts.body_doc.take() {
            inner.push(Doc::space());
            inner.push(body);
        }
        Some(Doc::group(Doc::concat(vec![
            Doc::space(),
            Doc::text(":"),
            Doc::continuation_indent(Doc::concat(vec![Doc::Line, Doc::group(Doc::concat(inner))])),
        ])))
    } else {
        None
    };

    if let Some(st) = supertypes_group {
        full_header.push(st);
    }

    let has_where = parts.where_doc.is_some();
    if let Some(wh) = parts.where_doc {
        let last_st_is_by = node.children(&mut node.walk()).any(|c| {
            c.kind() == "delegation_specifier"
                && c.children(&mut c.walk())
                    .any(|cc| cc.kind() == "explicit_delegation" || cc.kind() == "by")
        });
        if last_st_is_by {
            full_header.push(Doc::continuation_indent(Doc::concat(vec![
                Doc::HardLine,
                wh,
            ])));
        } else {
            full_header.push(Doc::continuation_indent(Doc::concat(vec![Doc::Line, wh])));
        }
        full_header.push(Doc::space());
    }

    let header_doc = Doc::group(Doc::concat(full_header));
    if let Some(body) = parts.body_doc {
        if header_doc.ends_with_newline() || has_where {
            Doc::concat(vec![header_doc, body])
        } else {
            Doc::concat(vec![header_doc, Doc::space(), body])
        }
    } else {
        header_doc
    }
}
