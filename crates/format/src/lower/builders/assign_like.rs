use tree_sitter::Node;

use crate::doc::{Doc, IndentKind};
use crate::lower::context::LowerContext;

pub use super::rhs_predicate::{is_block_like_rhs, is_block_like_rhs_ext, is_scoping_call};

/// Formats an assignment-like construct (`LHS <op> RHS`) using declarative grouping and conditional indent.
pub fn format_assign_like<'a>(
    lhs_doc: Doc<'a>,
    op: &'a str,
    rhs_node: Node<'a>,
    rhs_doc: Doc<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    format_assign_like_ext(lhs_doc, op, rhs_node, rhs_doc, false, ctx)
}

pub fn format_assign_like_ext<'a>(
    lhs_doc: Doc<'a>,
    op: &'a str,
    rhs_node: Node<'a>,
    rhs_doc: Doc<'a>,
    is_destructuring: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let is_block = is_block_like_rhs_ext(&rhs_node, is_destructuring, ctx);
    if is_destructuring && is_block && rhs_node.kind() == "call_expression" {
        let lhs_is_multiline = rhs_node
            .parent()
            .and_then(|p| {
                p.children(&mut p.walk()).find(|c| {
                    c.kind() == "multi_variable_declaration" || c.kind() == "variable_declaration"
                })
            })
            .is_some_and(|d| d.start_position().row != d.end_position().row);
        if lhs_is_multiline {
            let mut parts = vec![lhs_doc];
            if !parts[0].ends_with_newline() {
                parts.push(Doc::space());
            }
            parts.push(Doc::concat(vec![Doc::text(op), Doc::space(), rhs_doc]));
            return Doc::concat(parts);
        }
    }
    let rhs_group_id = ctx.next_group_id();
    let wrapped_rhs = if is_block {
        Doc::fits_first_line(rhs_doc)
    } else {
        rhs_doc
    };
    let mut parts = vec![lhs_doc];
    if !parts[0].ends_with_newline() {
        parts.push(Doc::space());
    }
    parts.push(Doc::group_with_id(
        rhs_group_id,
        Doc::concat(vec![
            Doc::text(op),
            Doc::indent_if_group_breaks(
                rhs_group_id,
                IndentKind::Continuation,
                Doc::concat(vec![Doc::Line, wrapped_rhs]),
            ),
        ]),
    ));
    Doc::concat(parts)
}

/// Formats an assignment-like construct where comments precede the RHS.
pub fn format_assign_with_comments<'a>(
    lhs_doc: Doc<'a>,
    op: &'a str,
    op_node: Option<Node<'a>>,
    comments: &[Node<'a>],
    rhs_node: Node<'a>,
    rhs_doc: Doc<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    format_assign_with_comments_ext(
        lhs_doc, op, op_node, comments, rhs_node, rhs_doc, false, ctx, lower_fn,
    )
}

fn build_assign_comments_rhs<'a>(
    comments: &[Node<'a>],
    newline_before_comments: bool,
    rhs_node: &Node<'a>,
    rhs_doc: Doc<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut parts = Vec::new();
    for (idx, c) in comments.iter().enumerate() {
        let has_newline = if idx == 0 {
            newline_before_comments
        } else {
            ctx.has_user_newline_between(&comments[idx - 1], c)
        };
        if has_newline {
            parts.push(Doc::HardLine);
        } else if idx > 0 {
            parts.push(Doc::space());
        }
        parts.push(lower_fn(*c, ctx));
    }
    let last_c = comments.last().unwrap();
    if ctx.has_user_newline_between(last_c, rhs_node) {
        parts.push(Doc::HardLine);
    } else {
        parts.push(Doc::space());
    }
    parts.push(rhs_doc);
    Doc::concat(parts)
}

pub fn format_assign_with_comments_ext<'a>(
    lhs_doc: Doc<'a>,
    op: &'a str,
    op_node: Option<Node<'a>>,
    comments: &[Node<'a>],
    rhs_node: Node<'a>,
    rhs_doc: Doc<'a>,
    is_destructuring: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if comments.is_empty() {
        return format_assign_like_ext(lhs_doc, op, rhs_node, rhs_doc, is_destructuring, ctx);
    }

    let has_line_comment = comments.iter().any(|c| {
        c.kind() == "line_comment" || (c.kind() == "comment" && ctx.node_text(c).starts_with("//"))
    });

    let first_c = &comments[0];
    let newline_before_comments = op_node
        .as_ref()
        .is_some_and(|op_n| ctx.has_user_newline_between(op_n, first_c));

    let is_block = !has_line_comment && is_block_like_rhs_ext(&rhs_node, is_destructuring, ctx);
    let full_rhs = build_assign_comments_rhs(
        comments,
        newline_before_comments,
        &rhs_node,
        rhs_doc,
        ctx,
        lower_fn,
    );

    if has_line_comment {
        let gid = ctx.next_group_id();
        Doc::concat(vec![
            lhs_doc,
            Doc::space(),
            Doc::group_with_id(
                gid,
                Doc::concat(vec![
                    Doc::text(op),
                    if !newline_before_comments {
                        Doc::space()
                    } else {
                        Doc::empty()
                    },
                    Doc::indent_if_group_breaks(gid, IndentKind::Continuation, full_rhs),
                ]),
            ),
        ])
    } else if newline_before_comments && is_block {
        Doc::concat(vec![lhs_doc, Doc::space(), Doc::text(op), full_rhs])
    } else {
        let rhs_group_id = ctx.next_group_id();
        let wrapped_rhs = if is_block {
            Doc::fits_first_line(full_rhs)
        } else {
            full_rhs
        };
        Doc::concat(vec![
            lhs_doc,
            Doc::space(),
            Doc::group_with_id(
                rhs_group_id,
                Doc::concat(vec![
                    Doc::text(op),
                    Doc::indent_if_group_breaks(
                        rhs_group_id,
                        IndentKind::Continuation,
                        Doc::concat(vec![Doc::Line, wrapped_rhs]),
                    ),
                ]),
            ),
        ])
    }
}
