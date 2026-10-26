use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{BlockItem, format_assign_like, format_braced_block};
use crate::lower::context::LowerContext;

fn is_braced_body(node: &Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "when_expression" {
        return false;
    }
    if kind == "statements" || kind == "block" || kind == "function_body" {
        return true;
    }
    if kind == "control_structure_body" {
        let mut cursor = node.walk();
        return node
            .children(&mut cursor)
            .any(|c| c.kind() == "{" || is_braced_body(&c));
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| {
        matches!(
            c.kind(),
            "{" | "block" | "lambda_literal" | "annotated_lambda"
        )
    })
}

pub fn lower_when_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut subject = None;
    let mut entries: Vec<BlockItem<'a>> = Vec::new();
    let mut prev_child: Option<Node<'a>> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "when_subject" => subject = Some(child),
            "when_entry" | "comment" | "line_comment" | "multiline_comment" => {
                if ctx.style.preset == crate::options::StylePreset::Kotlinlang
                    && kind == "line_comment"
                    && let Some(prev) =
                        prev_child.and_then(crate::rules::comments::find_trailing_line_comment)
                    && crate::rules::comments::should_merge_wrapped_comment(&prev, &child, ctx)
                    && let Some(last_entry) = entries.last_mut()
                    && crate::rules::comments::try_append_to_trailing_line_comment(
                        &mut last_entry.doc,
                        ctx.node_text(&child),
                    )
                {
                    prev_child = Some(child);
                    continue;
                }
                let has_blank = prev_child.is_some_and(|p| ctx.has_blank_line_between(&p, &child));
                entries
                    .push(BlockItem::new(lower_fn(child, ctx)).with_blank_line_before(has_blank));
                prev_child = Some(child);
            }
            _ => {}
        }
    }

    let mut header = vec![Doc::text("when")];
    if let Some(sub) = subject {
        header.push(Doc::space());
        header.push(lower_fn(sub, ctx));
    }

    let body_doc = format_braced_block(entries, Vec::new());
    Doc::concat(vec![Doc::concat(header), Doc::space(), body_doc])
}

pub fn lower_when_subject<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut inner_docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" || kind == ")" {
            continue;
        }
        if kind == "=" {
            inner_docs.push(Doc::text(" = "));
        } else if kind == "val" || kind == "var" {
            let kw = ctx.node_text(&child);
            inner_docs.push(Doc::smol(common::concat_smol_str!(kw, " ")));
        } else {
            inner_docs.push(lower_fn(child, ctx));
        }
    }
    if ctx.style.soft_break_inside_condition_parens {
        Doc::group(Doc::concat(vec![
            Doc::text("("),
            Doc::continuation_indent(Doc::concat(vec![Doc::SoftLine, Doc::concat(inner_docs)])),
            Doc::SoftLine,
            Doc::text(")"),
        ]))
    } else {
        Doc::group(Doc::concat(vec![
            Doc::text("("),
            Doc::concat(inner_docs),
            Doc::text(")"),
        ]))
    }
}

pub fn lower_guard_condition<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut expr_doc = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "if" {
            expr_doc = Some(lower_fn(child, ctx));
        }
    }
    if let Some(doc) = expr_doc {
        Doc::concat(vec![Doc::text("if "), doc])
    } else {
        Doc::text("if")
    }
}

fn handle_when_condition_comma<'a>(
    child: Node<'a>,
    condition_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let is_trailing = child.next_sibling().is_none_or(|next| {
        next.kind() == "->"
            || (matches!(
                next.kind(),
                "comment" | "line_comment" | "multiline_comment"
            ) && next.next_sibling().is_none_or(|n2| n2.kind() == "->"))
    });
    if is_trailing {
        if ctx.options.trailing_comma_strategy != crate::options::TrailingCommaStrategy::Complete {
            condition_docs.push(Doc::text(","));
            let has_user_newline = child
                .next_sibling()
                .is_some_and(|next| ctx.has_user_newline_between(&child, &next));
            if has_user_newline {
                condition_docs.push(Doc::HardLine);
            }
        }
        return;
    }
    let next_is_same_line_comment = child.next_sibling().is_some_and(|next| {
        matches!(
            next.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) && !ctx.has_user_newline_between(&child, &next)
    });
    let has_user_newline = child
        .next_sibling()
        .is_some_and(|next| ctx.has_user_newline_between(&child, &next));
    condition_docs.push(Doc::text(","));
    if next_is_same_line_comment {
        condition_docs.push(Doc::space());
    } else if has_user_newline {
        condition_docs.push(Doc::HardLine);
    } else {
        condition_docs.push(Doc::Line);
    }
}

fn format_when_entry_body<'a>(
    conditions_doc: Doc<'a>,
    body: Node<'a>,
    pending_comments: Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let body_doc = lower_fn(body, ctx);
    if !pending_comments.is_empty() {
        let mut parts = Vec::new();
        for c in pending_comments {
            parts.push(c);
            parts.push(Doc::HardLine);
        }
        parts.push(body_doc);
        Doc::concat(vec![
            conditions_doc,
            Doc::text(" ->"),
            Doc::continuation_indent(Doc::concat(vec![Doc::HardLine, Doc::concat(parts)])),
        ])
    } else if body.kind() == "when_expression" {
        Doc::concat(vec![
            conditions_doc,
            Doc::text(" ->"),
            Doc::continuation_indent(Doc::concat(vec![Doc::HardLine, body_doc])),
        ])
    } else if is_braced_body(&body) {
        Doc::concat(vec![conditions_doc, Doc::text(" -> "), body_doc])
    } else {
        format_assign_like(conditions_doc, "->", body, body_doc, ctx)
    }
}

fn push_when_after_body_comment<'a>(
    child: Node<'a>,
    prev_after_body: &mut Option<Node<'a>>,
    body_node: Option<Node<'a>>,
    after_body_comments: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    let anchor = (*prev_after_body).or(body_node);
    if crate::rules::comments::try_merge_continuation_comment(
        anchor,
        &child,
        after_body_comments,
        ctx,
    ) {
        *prev_after_body = Some(child);
        return;
    }
    let is_same_line = anchor.is_some_and(|b| !ctx.has_user_newline_between(&b, &child));
    let has_blank = anchor.is_some_and(|b| ctx.has_blank_line_between(&b, &child));
    if is_same_line {
        after_body_comments.push(Doc::space());
        let c_doc = if kind == "line_comment" {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        after_body_comments.push(c_doc);
    } else {
        after_body_comments.push(if has_blank {
            Doc::EmptyLine
        } else {
            Doc::HardLine
        });
        after_body_comments.push(lower_fn(child, ctx));
    }
    *prev_after_body = Some(child);
}

fn push_when_condition_child<'a>(
    child: Node<'a>,
    condition_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    if kind == "," {
        handle_when_condition_comma(child, condition_docs, ctx);
    } else if kind == "guard_condition" {
        condition_docs.push(Doc::space());
        condition_docs.push(lower_guard_condition(child, ctx, lower_fn));
    } else if kind == "line_comment" {
        let prev = child.prev_sibling();
        let is_same_line = prev.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
        if is_same_line {
            if !condition_docs.last().is_some_and(|d| d.ends_with_space()) {
                condition_docs.push(Doc::space());
            }
        } else if !condition_docs.is_empty()
            && !condition_docs.last().is_some_and(|d| d.ends_with_newline())
        {
            condition_docs.push(Doc::HardLine);
        }
        condition_docs.push(lower_fn(child, ctx));
        condition_docs.push(Doc::HardLine);
    } else {
        if matches!(kind, "comment" | "multiline_comment")
            && !condition_docs.is_empty()
            && !condition_docs
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
        {
            condition_docs.push(Doc::space());
        }
        condition_docs.push(lower_fn(child, ctx));
    }
}

pub fn lower_when_entry<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut condition_docs = Vec::new();
    let mut pending_comments = Vec::new();
    let mut after_body_comments = Vec::new();
    let mut body_node = None;
    let mut prev_after_body = None;
    let mut in_conditions = true;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "->" {
            in_conditions = false;
            continue;
        }
        if in_conditions {
            push_when_condition_child(child, &mut condition_docs, ctx, lower_fn);
        } else if body_node.is_none() {
            if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                pending_comments.push(lower_fn(child, ctx));
            } else {
                body_node = Some(child);
                prev_after_body = Some(child);
            }
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            push_when_after_body_comment(
                child,
                &mut prev_after_body,
                body_node,
                &mut after_body_comments,
                ctx,
                lower_fn,
            );
        }
    }

    let conditions_doc = Doc::concat(condition_docs);
    let Some(body) = body_node else {
        return Doc::group(Doc::concat(vec![conditions_doc, Doc::text(" ->")]));
    };

    let entry_doc = format_when_entry_body(conditions_doc, body, pending_comments, ctx, lower_fn);
    if !after_body_comments.is_empty() {
        let mut all = vec![Doc::group(entry_doc)];
        all.extend(after_body_comments);
        Doc::concat(all)
    } else {
        Doc::group(entry_doc)
    }
}
