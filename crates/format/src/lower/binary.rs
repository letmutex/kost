use tree_sitter::Node;

use crate::doc::{Doc, GroupId, IndentKind};
use crate::lower::context::LowerContext;

pub fn is_binary_operator(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "&&"
            | "||"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | "<="
            | ">"
            | ">="
            | "?:"
            | "as"
            | "as?"
            | "is"
            | "!is"
            | ".."
            | "..<"
            | "in"
            | "!in"
            | "custom_operation_symbol"
    )
}

pub fn lower_binary_expression_with_prefix<'a>(
    node: Node<'a>,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let kind = node.kind();
    if super::binary_chain::is_chainable_binary(kind) {
        return super::binary_chain::format_chainable_binary(node, kind, prefix_doc, ctx, lower_fn);
    }
    lower_binary_expression_internal(node, prefix_doc, ctx, lower_fn)
}

pub fn lower_binary_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let kind = node.kind();
    if super::binary_chain::is_chainable_binary(kind) {
        let parent_is_same = node.parent().is_some_and(|p| p.kind() == kind);
        if !parent_is_same {
            return super::binary_chain::format_chainable_binary(node, kind, None, ctx, lower_fn);
        }
    }
    lower_binary_expression_internal(node, None, ctx, lower_fn)
}

fn lower_binary_expression_internal<'a>(
    node: Node<'a>,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut left = None;
    let mut op_node = None;
    let mut right = None;
    let mut comments_after_left = Vec::new();
    let mut comments_after_op = Vec::new();
    let mut comments_after_right = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            if right.is_some() {
                comments_after_right.push(child);
            } else if op_node.is_some() {
                comments_after_op.push(child);
            } else if left.is_some() {
                comments_after_left.push(child);
            }
            continue;
        }
        if left.is_none() {
            left = Some(child);
        } else if op_node.is_none()
            && (is_binary_operator(&child) || node.kind() == "infix_expression")
        {
            op_node = Some(child);
        } else if right.is_none() {
            right = Some(child);
        }
    }

    let (Some(l), Some(op), Some(r)) = (left, op_node, right) else {
        return Doc::text(ctx.node_text(&node));
    };

    format_simple_binary(
        l,
        op,
        r,
        comments_after_left,
        comments_after_op,
        comments_after_right,
        prefix_doc,
        ctx,
        lower_fn,
    )
}

fn assemble_binary_operand_with_comments<'a>(
    node: Node<'a>,
    comments: Vec<Node<'a>>,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let base = lower_fn(node, ctx);
    let mut parts = vec![base];
    parts.extend(super::binary_chain::lower_trailing_comments(
        &node, comments, ctx, lower_fn,
    ));
    if let Some(prefix) = prefix_doc {
        Doc::group(Doc::concat(vec![prefix, Doc::Line, Doc::concat(parts)]))
    } else {
        Doc::concat(parts)
    }
}

fn has_multiline_call_value_args(node: &Node<'_>, max_width: usize) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "value_arguments" {
            return child.start_position().row != child.end_position().row
                || child.start_position().column + child.byte_range().len() > max_width;
        }
        if child.kind() == "call_suffix" {
            let mut sc = child.walk();
            for sub in child.children(&mut sc) {
                if sub.kind() == "value_arguments" {
                    return sub.start_position().row != sub.end_position().row
                        || sub.start_position().column + sub.byte_range().len() > max_width;
                }
            }
        }
    }
    false
}

fn format_simple_binary<'a>(
    l: Node<'a>,
    op: Node<'a>,
    r: Node<'a>,
    comments_after_left: Vec<Node<'a>>,
    comments_after_op: Vec<Node<'a>>,
    comments_after_right: Vec<Node<'a>>,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let op_text = ctx.node_text(&op);

    if op_text == ".." || op_text == "..<" {
        return Doc::concat(vec![lower_fn(l, ctx), Doc::text(op_text), lower_fn(r, ctx)]);
    }

    let has_left_trailing = !comments_after_left.is_empty();
    let full_left =
        assemble_binary_operand_with_comments(l, comments_after_left, prefix_doc, ctx, lower_fn);
    let full_right =
        assemble_binary_operand_with_comments(r, comments_after_right, None, ctx, lower_fn);

    if op_text == "?:" {
        return super::binary_chain::format_elvis_operator_doc(
            op,
            comments_after_op,
            full_left,
            full_right,
            has_left_trailing,
            ctx,
            lower_fn,
        );
    }

    let gid = ctx.next_group_id();
    let mut op_parts = vec![Doc::space(), Doc::text(op_text)];
    let has_op_trailing = !comments_after_op.is_empty();
    op_parts.extend(super::binary_chain::lower_trailing_comments(
        &op,
        comments_after_op,
        ctx,
        lower_fn,
    ));

    let rhs_for_group = if full_right.ends_with_brace()
        && !has_multiline_call_value_args(&r, ctx.options.max_width)
    {
        Doc::fits_first_line(full_right)
    } else {
        full_right
    };
    let indent_right = if has_op_trailing {
        Doc::indent_if_group_breaks(gid, IndentKind::Continuation, rhs_for_group)
    } else {
        Doc::indent_if_group_breaks(
            gid,
            IndentKind::Continuation,
            Doc::concat(vec![Doc::Line, rhs_for_group]),
        )
    };

    Doc::group_with_id(
        gid,
        Doc::concat(vec![full_left, Doc::concat(op_parts), indent_right]),
    )
}

fn is_qualified_expr(node: &Node<'_>) -> bool {
    node.kind() == "navigation_expression"
        || (node.kind() == "call_expression"
            && node
                .child(0)
                .is_some_and(|c| c.kind() == "navigation_expression"))
}

fn assemble_as_or_check_doc<'a>(
    left_doc: Doc<'a>,
    op_text: String,
    right_part: Doc<'a>,
    break_before: bool,
    open_group_before_left: bool,
    gid: GroupId,
) -> Doc<'a> {
    if !break_before {
        Doc::concat(vec![
            left_doc,
            Doc::space(),
            Doc::group_with_id(
                gid,
                Doc::concat(vec![
                    Doc::smol(op_text),
                    Doc::indent_if_group_breaks(gid, IndentKind::Continuation, right_part),
                ]),
            ),
        ])
    } else if open_group_before_left {
        Doc::group_with_id(
            gid,
            Doc::concat(vec![
                left_doc,
                Doc::indent_if_group_breaks(
                    gid,
                    IndentKind::Continuation,
                    Doc::concat(vec![Doc::Line, Doc::smol(op_text), right_part]),
                ),
            ]),
        )
    } else {
        Doc::concat(vec![
            left_doc,
            Doc::group_with_id(
                gid,
                Doc::indent_if_group_breaks(
                    gid,
                    IndentKind::Continuation,
                    Doc::concat(vec![Doc::Line, Doc::smol(op_text), right_part]),
                ),
            ),
        ])
    }
}

pub fn lower_as_or_check_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut left = None;
    let mut op_tokens = Vec::new();
    let mut right_nodes = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if left.is_none() {
            left = Some(child);
        } else if matches!(kind, "as" | "as?" | "is" | "!is" | "!" | "in" | "!in") {
            op_tokens.push(ctx.node_text(&child));
        } else {
            right_nodes.push(child);
        }
    }

    let Some(l) = left else {
        return Doc::text(ctx.node_text(&node));
    };

    let op_text = op_tokens.join("");
    let left_doc = lower_fn(l, ctx);
    let mut right_docs = Vec::new();
    for r in right_nodes {
        right_docs.push(lower_fn(r, ctx));
    }
    let right_doc = Doc::concat(right_docs);

    let is_as = op_text.starts_with("as");
    let parent_is_arg = node
        .parent()
        .is_some_and(|p| matches!(p.kind(), "value_argument" | "parenthesized_expression"));
    let break_before = is_as || parent_is_arg;
    let open_group_before_left = !is_qualified_expr(&l);

    let gid = ctx.next_group_id();
    let right_part = Doc::group(Doc::concat(vec![Doc::Line, right_doc]));

    assemble_as_or_check_doc(
        left_doc,
        op_text,
        right_part,
        break_before,
        open_group_before_left,
        gid,
    )
}
