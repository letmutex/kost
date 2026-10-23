use tree_sitter::Node;

use crate::doc::{Doc, IndentKind};
use crate::lower::context::LowerContext;

pub(crate) fn is_chainable_binary(kind: &str) -> bool {
    matches!(
        kind,
        "conjunction_expression"
            | "disjunction_expression"
            | "additive_expression"
            | "multiplicative_expression"
    )
}

fn split_annotated_binary_chain<'a>(
    node: Node<'a>,
    parent_kind: &str,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Option<(Doc<'a>, Node<'a>, Vec<Node<'a>>)> {
    let mut curr = node;
    let mut prefix_docs = Vec::new();
    let mut last_anno: Option<Node<'a>> = None;
    loop {
        if curr.kind() != "prefix_expression" {
            return None;
        }
        let first = curr.child(0)?;
        if !matches!(first.kind(), "annotation" | "label") {
            return None;
        }
        let second = curr.child(1)?;
        if let Some(last) = last_anno {
            if ctx.has_user_newline_between(&last, &first) {
                prefix_docs.push(Doc::HardLine);
            } else {
                prefix_docs.push(Doc::space());
            }
        }
        prefix_docs.push(lower_fn(first, ctx));
        last_anno = Some(first);
        if second.kind() == "prefix_expression"
            && second
                .child(0)
                .is_some_and(|c| matches!(c.kind(), "annotation" | "label"))
        {
            curr = second;
            continue;
        }
        if second.kind() == parent_kind {
            let mut sub_elems = Vec::new();
            collect_binary_elements(second, parent_kind, &mut sub_elems);
            if !sub_elems.is_empty() {
                let first_elem = sub_elems.remove(0);
                let first_doc = lower_fn(first_elem, ctx);
                let combined = Doc::group(Doc::concat(vec![
                    Doc::concat(prefix_docs),
                    Doc::Line,
                    first_doc,
                ]));
                return Some((combined, first_elem, sub_elems));
            }
        }
        return None;
    }
}

pub(crate) fn collect_binary_elements<'a>(
    node: Node<'a>,
    parent_kind: &str,
    elements: &mut Vec<Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == parent_kind {
            collect_binary_elements(child, parent_kind, elements);
        } else {
            elements.push(child);
        }
    }
}

fn push_chainable_rhs_operand<'a>(
    node: &Node<'a>,
    elem: Node<'a>,
    elem_doc: Doc<'a>,
    prev_elem: Option<Node<'a>>,
    op_on_newline: bool,
    gid: crate::doc::GroupId,
    parts: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let prev_is_comment = prev_elem
        .as_ref()
        .is_some_and(|p| matches!(p.kind(), "comment" | "line_comment" | "multiline_comment"));
    let is_inline_after_comment = prev_is_comment
        && prev_elem
            .as_ref()
            .is_some_and(|p| !ctx.has_user_newline_between(p, &elem));
    if is_inline_after_comment {
        parts.push(Doc::indent_if_group_breaks(
            gid,
            IndentKind::Continuation,
            elem_doc,
        ));
    } else if prev_is_comment
        && prev_elem
            .as_ref()
            .is_some_and(|p| ctx.has_user_newline_between(p, &elem))
    {
        parts.push(Doc::indent_if_group_breaks(
            gid,
            IndentKind::Continuation,
            Doc::concat(vec![Doc::HardLine, elem_doc]),
        ));
    } else if op_on_newline {
        parts.push(Doc::space());
        parts.push(elem_doc);
    } else {
        let is_double_continuation = ctx.options.block_indent == 4
            && node.parent().is_some_and(|p| {
                p.kind() == "parenthesized_expression"
                    && p.parent().is_some_and(|gp| {
                        matches!(gp.kind(), "navigation_expression" | "call_expression")
                    })
            });
        let indented = if is_double_continuation {
            Doc::indent_if_group_breaks(
                gid,
                IndentKind::Continuation,
                Doc::indent_if_group_breaks(
                    gid,
                    IndentKind::Continuation,
                    Doc::concat(vec![Doc::Line, elem_doc]),
                ),
            )
        } else {
            Doc::indent_if_group_breaks(
                gid,
                IndentKind::Continuation,
                Doc::concat(vec![Doc::Line, elem_doc]),
            )
        };
        parts.push(indented);
    }
}

pub(crate) fn format_chainable_binary<'a>(
    node: Node<'a>,
    kind: &str,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut elements = Vec::new();
    collect_binary_elements(node, kind, &mut elements);
    let gid = ctx.next_group_id();
    let mut parts = Vec::new();
    let mut prev_was_op = false;
    let mut op_on_newline = false;
    let mut prev_elem: Option<Node<'a>> = None;
    let mut idx = 0;
    while idx < elements.len() {
        let elem = elements[idx];
        idx += 1;
        if matches!(
            elem.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            if crate::rules::comments::try_merge_continuation_comment(
                prev_elem, &elem, &mut parts, ctx,
            ) {
                prev_elem = Some(elem);
                continue;
            }
            let is_inline = prev_elem
                .as_ref()
                .is_some_and(|p| !ctx.has_user_newline_between(p, &elem));
            let c_doc = if is_inline && elem.kind() == "line_comment" {
                Doc::line_comment(
                    ctx.node_text(&elem),
                    true,
                    ctx.style.align_wrapped_trailing_comments,
                )
            } else {
                lower_fn(elem, ctx)
            };
            if is_inline {
                parts.push(Doc::space());
                parts.push(c_doc);
            } else {
                parts.push(Doc::indent_if_group_breaks(
                    gid,
                    IndentKind::Continuation,
                    Doc::concat(vec![Doc::HardLine, c_doc]),
                ));
            }
        } else if super::binary::is_binary_operator(&elem) {
            let op_text = ctx.node_text(&elem);
            let is_newline_after_comment = prev_elem.as_ref().is_some_and(|p| {
                matches!(p.kind(), "comment" | "line_comment" | "multiline_comment")
                    && ctx.has_user_newline_between(p, &elem)
            });
            if is_newline_after_comment {
                parts.push(Doc::indent_if_group_breaks(
                    gid,
                    IndentKind::Continuation,
                    Doc::concat(vec![Doc::HardLine, Doc::text(op_text)]),
                ));
                op_on_newline = true;
            } else {
                parts.push(Doc::space());
                parts.push(Doc::text(op_text));
                op_on_newline = false;
            }
            prev_was_op = true;
        } else if prev_was_op {
            let (elem_doc, actual_elem) = if let Some((comb_doc, first_e, tail_elems)) =
                split_annotated_binary_chain(elem, kind, ctx, lower_fn)
            {
                elements.splice(idx..idx, tail_elems);
                (comb_doc, first_e)
            } else {
                (lower_fn(elem, ctx), elem)
            };
            push_chainable_rhs_operand(
                &node,
                elem,
                elem_doc,
                prev_elem,
                op_on_newline,
                gid,
                &mut parts,
                ctx,
            );
            prev_was_op = false;
            op_on_newline = false;
            prev_elem = Some(actual_elem);
            continue;
        } else {
            let elem_doc = lower_fn(elem, ctx);
            if let Some(prefix) = prefix_doc.clone() {
                parts.push(Doc::group(Doc::concat(vec![prefix, Doc::Line, elem_doc])));
            } else {
                parts.push(elem_doc);
            }
        }
        prev_elem = Some(elem);
    }
    Doc::group_with_id(gid, Doc::concat(parts))
}

pub(crate) fn lower_trailing_comments<'a>(
    anchor: &Node<'a>,
    comments: Vec<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut docs = Vec::new();
    let mut prev = *anchor;
    for c in comments {
        if ctx.style.align_wrapped_trailing_comments
            && c.kind() == "line_comment"
            && let Some(prev_c) = crate::rules::comments::find_trailing_line_comment(prev)
            && crate::rules::comments::should_merge_wrapped_comment(&prev_c, &c, ctx)
            && let Some(last_doc) = docs.last_mut()
            && crate::rules::comments::try_append_to_trailing_line_comment(
                last_doc,
                ctx.node_text(&c),
            )
        {
            prev = c;
            continue;
        }
        let is_inline = !ctx.has_user_newline_between(&prev, &c);
        if is_inline && matches!(c.kind(), "comment" | "line_comment") {
            docs.push(Doc::space());
            docs.push(Doc::line_comment(
                ctx.node_text(&c),
                true,
                ctx.style.align_wrapped_trailing_comments,
            ));
        } else {
            docs.push(Doc::HardLine);
            docs.push(lower_fn(c, ctx));
        }
        prev = c;
    }
    docs
}

pub(crate) fn format_elvis_operator_doc<'a>(
    op: Node<'a>,
    comments_after_op: Vec<Node<'a>>,
    full_left: Doc<'a>,
    full_right: Doc<'a>,
    has_left_trailing: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let gid = ctx.next_group_id();
    let mut op_parts = vec![Doc::text("?:")];
    let has_comments_after_op = !comments_after_op.is_empty();
    op_parts.extend(lower_trailing_comments(
        &op,
        comments_after_op,
        ctx,
        lower_fn,
    ));
    let right_sep = if has_comments_after_op {
        Doc::HardLine
    } else {
        Doc::space()
    };
    let elvis_right = if has_left_trailing {
        Doc::indent_if_group_breaks(
            gid,
            IndentKind::Continuation,
            Doc::concat(vec![Doc::concat(op_parts), right_sep, full_right]),
        )
    } else {
        Doc::indent_if_group_breaks(
            gid,
            IndentKind::Continuation,
            Doc::concat(vec![
                Doc::Line,
                Doc::concat(op_parts),
                right_sep,
                full_right,
            ]),
        )
    };
    if full_left.ends_with_brace() {
        Doc::concat(vec![full_left, Doc::group_with_id(gid, elvis_right)])
    } else {
        Doc::group_with_id(gid, Doc::concat(vec![full_left, elvis_right]))
    }
}
