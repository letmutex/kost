use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::delegates::{
    PropertyComment, build_property_init_section, call_with_multiline_args_fits,
    parse_comment_before_init, parse_property_delegate, property_first_line_fits,
};
use crate::lower::lower_node;
use crate::lower::types::lower_type_constraints;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_property_header<'a>(
    name_docs: Vec<Doc<'a>>,
    type_docs: Vec<Doc<'a>>,
    init_docs: Vec<Doc<'a>>,
    init_child: Option<Node<'a>>,
    op_str: Option<&'static str>,
    trailing_comment: Option<PropertyComment<'a>>,
    modifier_len: usize,
    node: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let name_doc = Doc::Concat(name_docs);
    let Some(op) = op_str else {
        return if type_docs.is_empty() {
            Doc::Group(Box::new(name_doc))
        } else {
            Doc::Group(Box::new(Doc::Concat(vec![
                name_doc,
                Doc::text(":"),
                Doc::continuation_indent(Doc::Concat(vec![Doc::Line, Doc::Concat(type_docs)])),
            ])))
        };
    };

    let flat_call = call_with_multiline_args_fits(node, init_child, ctx);
    let (init_section, flat_init) = build_property_init_section(
        init_docs,
        init_child,
        trailing_comment,
        ctx.options.block_indent == 4,
        flat_call,
    );

    let name_len = name_doc.flat_len();
    let type_len = if type_docs.is_empty() {
        0
    } else {
        2 + type_docs.iter().map(|d| d.flat_len()).sum::<usize>()
    };
    let init_flat_len = match &init_section {
        Doc::Concat(items) => items.iter().map(|d| d.flat_len()).sum::<usize>(),
        other => other.flat_len(),
    };
    let first_line_fits = property_first_line_fits(
        node,
        modifier_len,
        name_len,
        type_len,
        op,
        init_child,
        init_flat_len,
        flat_call,
        ctx,
    );

    if type_docs.is_empty() {
        if flat_init && !flat_call && !first_line_fits {
            let inner_init = match init_section {
                Doc::Concat(mut items) if items.first() == Some(&Doc::space()) => {
                    items.remove(0);
                    Doc::Concat(items)
                }
                other => other,
            };
            Doc::Concat(vec![
                name_doc,
                Doc::space(),
                Doc::text(op),
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, inner_init])),
            ])
        } else {
            let init_group = if flat_init {
                init_section
            } else {
                Doc::Group(Box::new(init_section))
            };
            Doc::Concat(vec![name_doc, Doc::space(), Doc::text(op), init_group])
        }
    } else if flat_init {
        format_flat_property_header(name_doc, type_docs, op, init_section, first_line_fits)
    } else {
        let type_and_op = Doc::Group(Box::new(Doc::Concat(vec![
            name_doc,
            Doc::text(":"),
            Doc::continuation_indent(Doc::Concat(vec![Doc::Line, Doc::Concat(type_docs)])),
            Doc::space(),
            Doc::text(op),
        ])));
        Doc::Group(Box::new(Doc::Concat(vec![type_and_op, init_section])))
    }
}

fn format_flat_property_header<'a>(
    name_doc: Doc<'a>,
    type_docs: Vec<Doc<'a>>,
    op: &'static str,
    init_section: Doc<'a>,
    first_line_fits: bool,
) -> Doc<'a> {
    if first_line_fits {
        Doc::Concat(vec![
            name_doc,
            Doc::text(": "),
            Doc::Concat(type_docs),
            Doc::space(),
            Doc::text(op),
            init_section,
        ])
    } else {
        let type_and_rest = Doc::continuation_indent(Doc::Concat(vec![
            Doc::Line,
            Doc::Concat(type_docs),
            Doc::space(),
            Doc::text(op),
            init_section,
        ]));
        Doc::Group(Box::new(Doc::Concat(vec![
            name_doc,
            Doc::text(":"),
            type_and_rest,
        ])))
    }
}

#[derive(Default)]
struct PropertyParts<'a> {
    modifier_docs: Vec<Doc<'a>>,
    name_docs: Vec<Doc<'a>>,
    type_docs: Vec<Doc<'a>>,
    init_docs: Vec<Doc<'a>>,
    init_child: Option<Node<'a>>,
    accessors: Vec<Doc<'a>>,
    where_doc: Option<Doc<'a>>,
    where_has_newline: bool,
    is_after_equals: bool,
    is_after_by: bool,
    trailing_comment: Option<PropertyComment<'a>>,
}

fn push_property_init_child<'a>(
    child: Node<'a>,
    eq_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    parts: &mut PropertyParts<'a>,
) {
    let kind = child.kind();
    let is_comment = matches!(kind, "comment" | "line_comment" | "multiline_comment");
    if is_comment && parts.init_docs.is_empty() && parts.trailing_comment.is_none() {
        parts.trailing_comment = Some(parse_comment_before_init(child, eq_node, ctx));
        return;
    }
    if parts.init_child.is_none() && !is_comment {
        parts.init_child = Some(child);
    }
    if !parts.init_docs.is_empty() {
        parts.init_docs.push(Doc::space());
    }
    parts.init_docs.push(lower_node(child, ctx));
}

fn parse_property_parts<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> PropertyParts<'a> {
    let mut parts = PropertyParts::default();
    let mut is_after_colon = false;
    let mut eq_node = None;
    let mut cursor = node.walk();
    let mut prev_child: Option<Node<'a>> = None;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "modifiers" {
            parts.modifier_docs.push(lower_node(child, ctx));
        } else if kind == "=" {
            parts.is_after_equals = true;
            eq_node = Some(child);
        } else if kind == ":" {
            is_after_colon = true;
        } else if kind == "type_constraints" {
            parts.where_has_newline =
                prev_child.is_some_and(|p| ctx.has_newline_between(&p, &child));
            parts.where_doc = Some(lower_type_constraints(child, ctx));
        } else if kind == "property_delegate" {
            parse_property_delegate(
                child,
                ctx,
                &mut parts.init_child,
                &mut parts.init_docs,
                &mut parts.trailing_comment,
            );
            parts.is_after_by = true;
        } else if parts.is_after_equals {
            push_property_init_child(child, eq_node, ctx, &mut parts);
        } else if kind == "getter" || kind == "setter" {
            parts.accessors.push(Doc::indent(Doc::Concat(vec![
                Doc::HardLine,
                Doc::Group(Box::new(lower_node(child, ctx))),
            ])));
        } else if kind == "variable_declaration" {
            parse_variable_declaration(child, ctx, &mut parts);
        } else if is_after_colon {
            push_spaced_doc(&mut parts.type_docs, lower_node(child, ctx));
        } else if kind == "." {
            parts.name_docs.push(Doc::text("."));
        } else {
            push_name_doc(&mut parts.name_docs, child, ctx);
        }
        prev_child = Some(child);
    }
    parts
}

fn push_spaced_doc<'a>(docs: &mut Vec<Doc<'a>>, doc: Doc<'a>) {
    if !docs.is_empty()
        && !docs
            .last()
            .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
    {
        docs.push(Doc::space());
    }
    docs.push(doc);
}

fn push_name_doc<'a>(docs: &mut Vec<Doc<'a>>, child: Node<'a>, ctx: &LowerContext<'a, '_>) {
    let kind = child.kind();
    if matches!(kind, ";" | "(" | ")" | "[" | "]" | ".")
        || matches!(docs.last(), Some(Doc::Text(".")))
    {
        docs.push(lower_node(child, ctx));
    } else {
        push_spaced_doc(docs, lower_node(child, ctx));
    }
}

fn parse_variable_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    parts: &mut PropertyParts<'a>,
) {
    let mut cursor = node.walk();
    let mut is_after_colon = false;
    for child in node.children(&mut cursor) {
        if child.kind() == ":" {
            is_after_colon = true;
        } else if is_after_colon {
            push_spaced_doc(&mut parts.type_docs, lower_node(child, ctx));
        } else {
            push_name_doc(&mut parts.name_docs, child, ctx);
        }
    }
}

pub fn lower_property_declaration<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let parts = parse_property_parts(node, ctx);
    let op_str = if parts.is_after_by {
        Some("by")
    } else if parts.is_after_equals {
        Some("=")
    } else {
        None
    };
    let modifier_len = parts
        .modifier_docs
        .iter()
        .map(|d| d.flat_len())
        .sum::<usize>();
    let header_doc = build_property_header(
        parts.name_docs,
        parts.type_docs,
        parts.init_docs,
        parts.init_child,
        op_str,
        parts.trailing_comment,
        modifier_len,
        &node,
        ctx,
    );

    let header_with_where = if let Some(w) = parts.where_doc {
        let line = if parts.where_has_newline {
            Doc::HardLine
        } else {
            Doc::Line
        };
        Doc::Group(Box::new(Doc::Concat(vec![
            header_doc,
            Doc::continuation_indent(Doc::Concat(vec![line, w])),
        ])))
    } else {
        header_doc
    };

    let full_doc = if parts.accessors.is_empty() {
        header_with_where
    } else {
        Doc::Concat(vec![header_with_where, Doc::Concat(parts.accessors)])
    };

    let with_modifiers = if parts.modifier_docs.is_empty() {
        full_doc
    } else {
        Doc::Concat(vec![Doc::Concat(parts.modifier_docs), full_doc])
    };
    if with_modifiers.has_hardline() {
        with_modifiers
    } else {
        Doc::Group(Box::new(with_modifiers))
    }
}
