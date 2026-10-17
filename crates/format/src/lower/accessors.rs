use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::comma_list::{CommaListItem, CommaListOptions, format_comma_list};
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

fn format_accessor_paren_items<'a>(
    paren_items: Vec<CommaListItem<'a>>,
    has_trailing_comma: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let has_user_newline = paren_items
        .iter()
        .any(|it: &CommaListItem<'a>| it.doc.has_hardline());
    format_comma_list(
        paren_items,
        has_trailing_comma,
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
    )
}

pub fn lower_getter_setter<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut head_docs = Vec::new();
    let mut in_parens = false;
    let mut paren_items = Vec::new();
    let mut has_trailing_comma = false;
    let mut body_node = None;
    let mut is_after_colon = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" {
            in_parens = true;
            continue;
        }
        if kind == ")" {
            in_parens = false;
            head_docs.push(format_accessor_paren_items(
                std::mem::take(&mut paren_items),
                has_trailing_comma,
                ctx,
            ));
            continue;
        }
        if in_parens {
            if kind == "," {
                has_trailing_comma = true;
                continue;
            }
            if !child.is_missing() && !child.byte_range().is_empty() {
                has_trailing_comma = false;
                paren_items.push(CommaListItem {
                    doc: lower_node(child, ctx),
                    trailing_comment: None,
                });
            }
            continue;
        }
        if kind == "function_body" || kind == "block" {
            body_node = Some(child);
            continue;
        }
        if kind == ":" {
            is_after_colon = true;
            continue;
        }
        if is_after_colon {
            head_docs.push(Doc::text(": "));
            head_docs.push(lower_node(child, ctx));
            is_after_colon = false;
            continue;
        }
        if !head_docs.is_empty()
            && !head_docs
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
        {
            head_docs.push(Doc::space());
        }
        head_docs.push(lower_node(child, ctx));
    }

    let header_doc = Doc::Group(Box::new(Doc::Concat(head_docs)));
    let Some(body) = body_node else {
        return header_doc;
    };

    let body_doc = lower_node(body, ctx);
    Doc::Concat(vec![
        header_doc,
        Doc::space(),
        Doc::Group(Box::new(body_doc)),
    ])
}

pub fn is_field_accessor(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    if node.kind() != "assignment" {
        return false;
    }
    let Some(lhs) = node.child(0) else {
        return false;
    };
    ctx.node_text(&lhs).trim() == "field"
}

pub fn is_top_level_accessor(
    node: &Node<'_>,
    prev: Option<Node<'_>>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let Some(prev) = prev else {
        return false;
    };
    if prev.kind() != "property_declaration" {
        return false;
    }
    if node.kind() == "call_expression"
        && let Some(callee) = node.child(0)
        && callee.kind() == "simple_identifier"
    {
        let name = ctx.node_text(&callee);
        return name == "get" || name == "set";
    }
    false
}
