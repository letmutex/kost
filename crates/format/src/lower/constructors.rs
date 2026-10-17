use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::expr::lower_statements;
use crate::lower::lists::{CommaListOptions, format_comma_list, lower_list_with_commas};
use crate::lower::lower_node;

pub fn lower_companion_object<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut has_body = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body" {
            has_body = true;
        }
        if !docs.is_empty() && child.kind() != ";" {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
    }
    if !has_body && node.next_named_sibling().is_some() {
        let after = ctx.source.get(node.end_byte()..).unwrap_or("");
        if after.trim_start().starts_with(';') {
            docs.push(Doc::text(";"));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_anonymous_initializer<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut inner_docs = Vec::new();
    let mut cursor = node.walk();
    let mut inside_braces = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" {
            inside_braces = true;
            continue;
        }
        if kind == "}" {
            break;
        }
        if inside_braces {
            inner_docs.push(lower_node(child, ctx));
        }
    }

    if inner_docs.is_empty() {
        Doc::text("init {}")
    } else {
        Doc::Concat(vec![
            Doc::text("init {"),
            Doc::indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(inner_docs)])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    }
}

pub fn primary_constructor_has_constructor_keyword(
    node: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            return true;
        }
        if child.kind() == "(" {
            let prefix_text = ctx
                .source
                .get(node.start_byte()..child.start_byte())
                .unwrap_or("");
            return prefix_text
                .split_whitespace()
                .any(|token| token == "constructor");
        }
    }
    false
}

struct PrimaryConstructorChildren<'a> {
    prefix_docs: Vec<Doc<'a>>,
    items: Vec<Doc<'a>>,
    trailing_docs: Vec<Doc<'a>>,
    has_trailing_comma: bool,
    open_paren_start: Option<usize>,
    close_paren_end: Option<usize>,
}

fn collect_primary_constructor_children<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> PrimaryConstructorChildren<'a> {
    let mut prefix_docs = Vec::new();
    let mut items = Vec::new();
    let mut trailing_docs = Vec::new();
    let mut has_trailing_comma = false;
    let mut inside_parens = false;
    let mut after_parens = false;
    let mut open_paren_start = None;
    let mut close_paren_end = None;

    let mut prev_child: Option<Node<'a>> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" {
            open_paren_start = Some(child.start_byte());
            inside_parens = true;
        } else if kind == ")" {
            close_paren_end = Some(child.end_byte());
            inside_parens = false;
            after_parens = true;
        } else if after_parens {
            if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                if prev_child.is_some_and(|p| ctx.has_newline_between(&p, &child)) {
                    trailing_docs.push(Doc::indent(Doc::HardLine));
                } else {
                    trailing_docs.push(Doc::space());
                }
                trailing_docs.push(lower_node(child, ctx));
            }
        } else if !inside_parens {
            if matches!(
                kind,
                "modifiers" | "comment" | "line_comment" | "multiline_comment"
            ) {
                prefix_docs.push(lower_node(child, ctx));
            }
        } else if kind == "," {
            has_trailing_comma = true;
            prev_child = Some(child);
            continue;
        } else {
            has_trailing_comma = false;
            items.push(lower_node(child, ctx));
        }
        prev_child = Some(child);
    }

    PrimaryConstructorChildren {
        prefix_docs,
        items,
        trailing_docs,
        has_trailing_comma,
        open_paren_start,
        close_paren_end,
    }
}

fn build_primary_constructor_head<'a>(
    prefix_docs: Vec<Doc<'a>>,
    has_constructor: bool,
) -> Vec<Doc<'a>> {
    let mut head_docs = Vec::new();
    if !prefix_docs.is_empty() {
        head_docs.extend(prefix_docs);
        if has_constructor {
            if !head_docs
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                head_docs.push(Doc::space());
            }
            head_docs.push(Doc::text("constructor"));
        }
    } else if has_constructor {
        head_docs.push(Doc::text("constructor"));
    }
    head_docs
}

pub fn lower_primary_constructor<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let has_constructor = primary_constructor_has_constructor_keyword(&node, ctx);
    let parsed = collect_primary_constructor_children(node, ctx);
    let mut head_docs = build_primary_constructor_head(parsed.prefix_docs, has_constructor);

    let has_user_newline = match (parsed.open_paren_start, parsed.close_paren_end) {
        (Some(start), Some(end)) => ctx.source.get(start..end).is_some_and(|s| s.contains('\n')),
        _ => ctx.node_text(&node).contains('\n'),
    };
    let items: Vec<crate::lower::comma_list::CommaListItem<'a>> =
        parsed.items.into_iter().map(From::from).collect();
    let list_doc = format_comma_list(
        items,
        parsed.has_trailing_comma,
        "(",
        ")",
        CommaListOptions {
            strategy: ctx.options.trailing_comma_strategy,
            is_declaration: true,
            has_user_newline,
            has_break_after_open: false,
            is_single_unnamed_lambda: false,
            is_kotlinlang: ctx.options.block_indent == 4,
        },
    );

    let list_group = Doc::Group(Box::new(list_doc));
    if !head_docs.is_empty() {
        head_docs.push(list_group);
        head_docs.extend(parsed.trailing_docs);
        Doc::Concat(head_docs)
    } else {
        let mut docs = vec![list_group];
        docs.extend(parsed.trailing_docs);
        Doc::Concat(docs)
    }
}

pub fn lower_secondary_constructor<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut prefix_docs = Vec::new();
    let mut params_doc = None;
    let mut params_multiline = false;
    let mut delegation_doc = None;
    let mut statements_node = None;
    let mut has_braces = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "context_receiver" => {
                prefix_docs.push(lower_node(child, ctx));
                prefix_docs.push(Doc::HardLine);
            }
            "modifiers" => {
                prefix_docs.push(lower_node(child, ctx));
            }
            "constructor" => {
                prefix_docs.push(Doc::text("constructor"));
            }
            "function_value_parameters" => {
                params_multiline = ctx.node_text(&child).contains('\n');
                params_doc = Some(lower_list_with_commas(child, ctx));
            }
            "constructor_delegation_call" => {
                delegation_doc = Some(lower_constructor_delegation_call(child, ctx));
            }
            "{" => {
                has_braces = true;
            }
            "statements" if child.named_child_count() > 0 => {
                statements_node = Some(child);
            }
            _ => {}
        }
    }

    let mut header_parts = prefix_docs;
    if let Some(params) = params_doc {
        header_parts.push(params);
    }
    let header = Doc::Concat(header_parts);

    let delegation_part = delegation_doc.map(|d| {
        if params_multiline {
            Doc::Concat(vec![Doc::space(), Doc::text(": "), d])
        } else {
            Doc::Group(Box::new(Doc::Concat(vec![
                Doc::space(),
                Doc::text(":"),
                Doc::continuation_indent(Doc::Concat(vec![Doc::Line, d])),
            ])))
        }
    });

    let mut all_head = vec![header];
    if let Some(del) = delegation_part {
        all_head.push(del);
    }
    let head_doc = Doc::Group(Box::new(Doc::Concat(all_head)));
    attach_secondary_constructor_body(head_doc, has_braces, statements_node, ctx)
}

fn attach_secondary_constructor_body<'a>(
    head_doc: Doc<'a>,
    has_braces: bool,
    statements_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if !has_braces {
        return head_doc;
    }
    if let Some(stmts) = statements_node {
        Doc::Concat(vec![
            head_doc,
            Doc::space(),
            Doc::text("{"),
            Doc::indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_statements(stmts, ctx),
            ])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    } else {
        Doc::Concat(vec![head_doc, Doc::space(), Doc::text("{}")])
    }
}

pub fn lower_constructor_delegation_call<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_node(child, ctx));
    }
    Doc::Concat(docs)
}

pub fn class_has_body(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for c in node.children(&mut cursor) {
        if matches!(c.kind(), "class_body" | "enum_class_body") {
            return true;
        }
    }
    false
}

pub fn is_orphan_constructor(node: Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    (node.kind() == "call_expression" || node.kind() == "secondary_constructor")
        && ctx.node_text(&node).starts_with("constructor")
}

pub fn is_comment_before_orphan_constructor(comment: Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut next = comment.next_sibling();
    while let Some(n) = next {
        if matches!(n.kind(), "comment" | "line_comment" | "multiline_comment") {
            next = n.next_sibling();
        } else {
            return is_orphan_constructor(n, ctx);
        }
    }
    false
}
