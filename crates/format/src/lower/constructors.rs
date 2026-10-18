use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lists::{CommaListItem, CommaListOptions, format_comma_list};
use crate::lower::lower_node;

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
    items: Vec<CommaListItem<'a>>,
    trailing_docs: Vec<Doc<'a>>,
    has_trailing_comma: bool,
    open_paren_start: Option<usize>,
    close_paren_end: Option<usize>,
}

fn push_after_paren_comment<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    trailing_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    if prev_child.is_some_and(|p| ctx.has_newline_between(&p, &child)) {
        trailing_docs.push(Doc::indent(Doc::HardLine));
    } else {
        trailing_docs.push(Doc::space());
    }
    trailing_docs.push(lower_node(child, ctx));
}

fn build_parameter_item<'a>(
    child: Node<'a>,
    pending_comments: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> CommaListItem<'a> {
    let item_doc = lower_node(child, ctx);
    let doc = if pending_comments.is_empty() {
        item_doc
    } else {
        let mut parts = Vec::new();
        for c in pending_comments.drain(..) {
            parts.push(c);
            parts.push(Doc::HardLine);
        }
        parts.push(item_doc);
        Doc::Concat(parts)
    };
    CommaListItem {
        doc,
        trailing_comment: None,
        is_comment: false,
    }
}

fn attach_same_line_comment<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    items: &mut [CommaListItem<'a>],
    ctx: &LowerContext<'a, '_>,
) {
    let last_item = items.last_mut().unwrap();
    let comment_doc = lower_node(child, ctx);
    if child.kind() != "line_comment" && prev_child.is_some_and(|p| p.kind() != ",") {
        last_item.doc = Doc::Concat(vec![last_item.doc.clone(), Doc::space(), comment_doc]);
    } else if let Some(existing) = last_item.trailing_comment.take() {
        last_item.trailing_comment = Some(Doc::Concat(vec![existing, Doc::space(), comment_doc]));
    } else {
        last_item.trailing_comment = Some(comment_doc);
    }
}

fn collect_primary_constructor_children<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> PrimaryConstructorChildren<'a> {
    let mut prefix_docs = Vec::new();
    let mut items: Vec<CommaListItem<'a>> = Vec::new();
    let mut trailing_docs = Vec::new();
    let mut has_trailing_comma = false;
    let mut inside_parens = false;
    let mut after_parens = false;
    let mut open_paren_start = None;
    let mut close_paren_end = None;

    let mut pending_comments = Vec::new();
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
            for c in pending_comments.drain(..) {
                items.push(CommaListItem {
                    doc: c,
                    trailing_comment: None,
                    is_comment: true,
                });
            }
        } else if after_parens {
            if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                push_after_paren_comment(child, prev_child, &mut trailing_docs, ctx);
            }
        } else if !inside_parens {
            if matches!(
                kind,
                "modifiers" | "comment" | "line_comment" | "multiline_comment"
            ) {
                prefix_docs.push(lower_node(child, ctx));
            }
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            let is_same_line =
                prev_child.is_some_and(|p| child.start_position().row == p.end_position().row);
            if is_same_line && !items.is_empty() && pending_comments.is_empty() {
                attach_same_line_comment(child, prev_child, &mut items, ctx);
            } else {
                pending_comments.push(lower_node(child, ctx));
            }
        } else if kind == "," {
            has_trailing_comma = true;
            prev_child = Some(child);
            continue;
        } else {
            has_trailing_comma = false;
            items.push(build_parameter_item(child, &mut pending_comments, ctx));
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
    let list_doc = format_comma_list(
        parsed.items,
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
    head_docs.push(list_group);
    head_docs.extend(parsed.trailing_docs);
    Doc::Concat(head_docs)
}
