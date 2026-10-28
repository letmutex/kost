use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{BlockItem, format_braced_block};
use crate::lower::context::LowerContext;
use crate::rules::comments::{should_merge_wrapped_comment, try_append_to_trailing_line_comment};

fn can_have_same_line_follower(prev: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let kind = prev.kind();
    if kind == "line_comment" {
        return false;
    }
    if kind == "comment" && ctx.node_text(prev).starts_with("//") {
        return false;
    }
    matches!(kind, "annotation" | "label")
}

fn merge_same_line_block_child<'a>(
    last_item: &mut BlockItem<'a>,
    child: Node<'a>,
    prev_child: Option<&Node<'a>>,
    child_starts_with_anno: bool,
    last_trailing_comment: &mut Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let prev_doc = std::mem::replace(&mut last_item.doc, Doc::empty());
    let prev_is_anno = prev_child.is_some_and(|p| p.kind() == "annotation");
    if prev_is_anno && child.kind() == "assignment" {
        *last_trailing_comment = None;
        last_item.doc =
            super::assignment::lower_assignment_with_prefix(child, Some(prev_doc), ctx, lower_fn);
        return;
    }
    let child_is_comment = matches!(
        child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let prev_is_comment =
        prev_child.is_some_and(|p| matches!(p.kind(), "comment" | "multiline_comment"));
    let sep = if !child_is_comment && !prev_is_comment && (prev_is_anno != child_starts_with_anno) {
        Doc::Line
    } else {
        Doc::space()
    };
    let child_doc = if child_is_comment && child.kind() == "line_comment" {
        Doc::line_comment(
            ctx.node_text(&child),
            true,
            ctx.style.align_wrapped_trailing_comments,
        )
    } else {
        lower_fn(child, ctx)
    };
    if child_is_comment && child.kind() == "line_comment" {
        *last_trailing_comment = Some(child);
    } else {
        *last_trailing_comment = None;
    }
    last_item.doc = Doc::group(Doc::concat(vec![prev_doc, sep, child_doc]));
}

fn push_block_child<'a>(
    items: &mut Vec<BlockItem<'a>>,
    child: Node<'a>,
    prev_child: &mut Option<Node<'a>>,
    last_trailing_comment: &mut Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
    has_semi_before: bool,
    semi_item_before: bool,
) {
    let prev_c = (*last_trailing_comment)
        .or_else(|| prev_child.and_then(crate::rules::comments::find_trailing_line_comment));
    if ctx.style.align_wrapped_trailing_comments
        && child.kind() == "line_comment"
        && let Some(prev) = prev_c
        && should_merge_wrapped_comment(&prev, &child, ctx)
        && let Some(last_item) = items.last_mut()
        && try_append_to_trailing_line_comment(&mut last_item.doc, ctx.node_text(&child))
    {
        *last_trailing_comment = Some(child);
        *prev_child = Some(child);
        return;
    }
    let child_is_comment = matches!(
        child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    if super::loops::try_attach_loop_body(
        items,
        child,
        prev_child,
        last_trailing_comment,
        has_semi_before,
        child_is_comment,
        ctx,
        lower_fn,
    ) {
        return;
    }
    let child_starts_with_anno = child.kind() == "annotation"
        || (child.kind() == "prefix_expression"
            && child.child(0).is_some_and(|c| c.kind() == "annotation"));
    let comment_followed_by_semi = child_is_comment
        && child.next_sibling().is_some_and(|next| {
            ctx.source
                .get(child.end_byte()..next.start_byte())
                .unwrap_or("")
                .contains(';')
        });
    let same_line = !has_semi_before
        && !comment_followed_by_semi
        && prev_child.as_ref().is_some_and(|p| {
            p.kind() != "line_comment"
                && !ctx.has_user_newline_between(p, &child)
                && (child_is_comment
                    || can_have_same_line_follower(p, ctx)
                    || (matches!(p.kind(), "comment" | "multiline_comment")
                        && child_starts_with_anno))
        });

    if same_line && let Some(last_item) = items.last_mut() {
        merge_same_line_block_child(
            last_item,
            child,
            prev_child.as_ref(),
            child_starts_with_anno,
            last_trailing_comment,
            ctx,
            lower_fn,
        );
    } else {
        *last_trailing_comment = None;
        let blank = !semi_item_before
            && prev_child
                .as_ref()
                .is_some_and(|p| ctx.has_blank_line_between(p, &child));
        items.push(BlockItem::new(lower_fn(child, ctx)).with_blank_line_before(blank));
    }
    *prev_child = Some(child);
}

fn starts_with_ambiguous_prefix(mut node: Node<'_>) -> bool {
    while matches!(
        node.kind(),
        "navigation_expression" | "call_expression" | "postfix_expression"
    ) {
        let Some(first) = node.child(0) else { break };
        node = first;
    }
    matches!(
        node.kind(),
        "lambda_literal" | "annotated_lambda" | "parenthesized_expression"
    )
}

fn handle_semicolon<'a>(
    items: &mut Vec<BlockItem<'a>>,
    semi_node: Node<'a>,
    prev_child: &mut Option<Node<'a>>,
    last_trailing_comment: &mut Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    *last_trailing_comment = None;
    if ctx.is_redundant_semicolon(&semi_node) {
        return;
    }
    let prev_is_comment = prev_child
        .as_ref()
        .is_some_and(|p| matches!(p.kind(), "comment" | "line_comment" | "multiline_comment"));
    if prev_is_comment
        || prev_child
            .as_ref()
            .is_some_and(|p| ctx.has_user_newline_between(p, &semi_node))
    {
        items.push(BlockItem::new(Doc::text(";")));
    } else if let Some(last) = items.last_mut() {
        let prev_doc = std::mem::replace(&mut last.doc, Doc::empty());
        last.doc = Doc::concat(vec![prev_doc, Doc::text(";")]);
    } else {
        items.push(BlockItem::new(Doc::text(";")));
    }
    *prev_child = Some(semi_node);
}

fn handle_trivia_semicolon<'a>(
    prev: &Node<'a>,
    s_child: &Node<'a>,
    items: &mut Vec<BlockItem<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> (bool, bool) {
    let trivia = ctx
        .source
        .get(prev.end_byte()..s_child.start_byte())
        .unwrap_or("");
    if !trivia.contains(';') {
        return (false, false);
    }
    let prev_is_comment = matches!(
        prev.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let s_child_is_comment = matches!(
        s_child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let is_ambiguous =
        prev_is_comment || s_child_is_comment || starts_with_ambiguous_prefix(*s_child);
    if !is_ambiguous {
        return (true, false);
    }
    let semi_after_newline = if let Some(semi_pos) = trivia.find(';') {
        trivia[..semi_pos].contains('\n')
    } else {
        false
    };
    if prev_is_comment || semi_after_newline {
        items.push(BlockItem::new(Doc::text(";")));
        (true, true)
    } else if let Some(last) = items.last_mut() {
        let prev_doc = std::mem::replace(&mut last.doc, Doc::empty());
        last.doc = Doc::concat(vec![prev_doc, Doc::text(";")]);
        (true, false)
    } else {
        items.push(BlockItem::new(Doc::text(";")));
        (true, true)
    }
}

fn format_block_items<'a>(items: Vec<BlockItem<'a>>, has_braces: bool, kind: &str) -> Doc<'a> {
    if has_braces || kind == "block" {
        format_braced_block(items, Vec::new())
    } else if items.len() == 1 {
        items.into_iter().next().unwrap().doc
    } else {
        let mut docs = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                if item.blank_line_before {
                    docs.push(Doc::EmptyLine);
                } else {
                    docs.push(Doc::HardLine);
                }
            }
            docs.push(item.doc);
        }
        Doc::concat(docs)
    }
}

pub fn lower_block_node<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut items = Vec::new();
    let mut prev_child = None;
    let mut last_trailing_comment = None;
    let mut has_braces = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" {
            has_braces = true;
            continue;
        }
        if kind == "}" || kind == "init" {
            continue;
        }
        if kind == ";" {
            handle_semicolon(
                &mut items,
                child,
                &mut prev_child,
                &mut last_trailing_comment,
                ctx,
            );
            continue;
        }

        if kind == "statements" {
            let mut sc = child.walk();
            for s_child in child.children(&mut sc) {
                if s_child.kind() == ";" {
                    handle_semicolon(
                        &mut items,
                        s_child,
                        &mut prev_child,
                        &mut last_trailing_comment,
                        ctx,
                    );
                    continue;
                }
                let (has_semi_before, semi_item_before) = if let Some(prev) = prev_child {
                    handle_trivia_semicolon(&prev, &s_child, &mut items, ctx)
                } else {
                    (false, false)
                };
                push_block_child(
                    &mut items,
                    s_child,
                    &mut prev_child,
                    &mut last_trailing_comment,
                    ctx,
                    lower_fn,
                    has_semi_before,
                    semi_item_before,
                );
            }
            continue;
        }

        push_block_child(
            &mut items,
            child,
            &mut prev_child,
            &mut last_trailing_comment,
            ctx,
            lower_fn,
            false,
            false,
        );
    }

    format_block_items(items, has_braces, node.kind())
}
