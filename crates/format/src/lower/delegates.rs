use tree_sitter::Node;

use crate::doc::Doc;
pub(crate) use crate::lower::call_args::{
    call_has_args, call_has_multiline_args, find_call_opening_paren_len, find_first_lambda,
    find_innermost_call, find_opening_call,
};
use crate::lower::context::LowerContext;
use crate::lower::{lower_comment, lower_node};

pub(crate) struct PropertyComment<'a> {
    pub(crate) doc: Doc<'a>,
    pub(crate) is_line_comment: bool,
    pub(crate) has_newline_before: bool,
}

pub(crate) fn parse_comment_before_init<'a>(
    node: Node<'a>,
    prev_op: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> PropertyComment<'a> {
    PropertyComment {
        doc: lower_comment(node, ctx),
        is_line_comment: ctx.node_text(&node).starts_with("//"),
        has_newline_before: prev_op.is_some_and(|p| ctx.has_newline_between(&p, &node)),
    }
}

pub(crate) fn parse_property_delegate<'a>(
    child: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    init_child: &mut Option<Node<'a>>,
    init_docs: &mut Vec<Doc<'a>>,
    trailing_comments: &mut Vec<PropertyComment<'a>>,
) {
    let mut by_cursor = child.walk();
    let mut by_node = None;
    for c in child.children(&mut by_cursor) {
        let kind = c.kind();
        if kind == "by" {
            by_node = Some(c);
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if init_child.is_none() {
                let prev_op = if trailing_comments.is_empty() {
                    by_node
                } else {
                    None
                };
                trailing_comments.push(parse_comment_before_init(c, prev_op, ctx));
            }
        } else if init_child.is_none() {
            *init_child = Some(c);
            init_docs.push(lower_node(c, ctx));
        }
    }
}

fn call_has_receiver(call: &Node<'_>) -> bool {
    call.child(0)
        .is_some_and(|callee| callee.kind() == "navigation_expression")
}

pub(crate) fn build_property_init_section<'a>(
    init_docs: Vec<Doc<'a>>,
    init_child: Option<Node<'a>>,
    trailing_comments: Vec<PropertyComment<'a>>,
    is_destructuring: bool,
    _is_untyped: bool,
    ctx: &LowerContext<'a, '_>,
) -> (Doc<'a>, bool) {
    let init_doc = Doc::Concat(init_docs);
    let is_kotlinlang = ctx.options.block_indent == 4;
    let flat_init = init_child.is_some_and(|r| {
        let opening = find_opening_call(r);
        let opening_has_receiver = opening.is_some_and(|c| call_has_receiver(&c));
        let is_top_level = ctx.compute_target_indent(&r) == 0;
        let has_lambda = find_first_lambda(r).is_some();
        crate::lower::scoping::is_scoping_chain_or_lambda(&r)
            || (!has_lambda
                && ((is_kotlinlang && is_top_level) || is_destructuring)
                && !opening_has_receiver
                && opening.is_some_and(|c| find_call_opening_paren_len(c, r, ctx).is_some()))
    });
    let is_raw_string = init_child.is_some_and(|r| {
        let mut curr = r;
        while curr.kind() == "call_expression" || curr.kind() == "navigation_expression" {
            if let Some(first) = curr.child(0) {
                curr = first;
            } else {
                break;
            }
        }
        matches!(curr.kind(), "multiline_string_literal")
            || (curr.kind() == "string_literal"
                && curr.start_position().row != curr.end_position().row)
    });
    let flat_init = flat_init && !is_raw_string && trailing_comments.is_empty();

    let doc = if !trailing_comments.is_empty() {
        let (first_is_line, first_has_newline) = (
            trailing_comments[0].is_line_comment,
            trailing_comments[0].has_newline_before,
        );
        let mut comment_items = Vec::with_capacity(trailing_comments.len() * 2);
        for (i, comment) in trailing_comments.into_iter().enumerate() {
            if i > 0 {
                comment_items.push(Doc::HardLine);
            }
            let comment_text = match comment.doc {
                Doc::Concat(mut items) => {
                    if items.last().is_some_and(|d| matches!(d, Doc::HardLine)) {
                        items.pop();
                    }
                    Doc::Concat(items)
                }
                c => c,
            };
            comment_items.push(comment_text);
        }
        let all_comments = Doc::Concat(comment_items);
        if first_is_line {
            if first_has_newline {
                Doc::continuation_indent(Doc::Concat(vec![
                    Doc::HardLine,
                    all_comments,
                    Doc::HardLine,
                    init_doc,
                ]))
            } else {
                Doc::Concat(vec![
                    Doc::space(),
                    all_comments,
                    Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, init_doc])),
                ])
            }
        } else if first_has_newline {
            Doc::Concat(vec![
                Doc::indent(Doc::Concat(vec![Doc::HardLine, all_comments, Doc::space()])),
                init_doc,
            ])
        } else {
            Doc::Concat(vec![Doc::space(), all_comments, Doc::space(), init_doc])
        }
    } else if is_raw_string {
        Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, init_doc]))
    } else if flat_init {
        Doc::Concat(vec![Doc::space(), init_doc])
    } else {
        Doc::if_break(
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, init_doc.clone()])),
            Doc::Concat(vec![Doc::space(), init_doc]),
        )
    };
    (doc, flat_init)
}

pub(crate) fn property_init_first_line_len(
    init_child: Option<Node<'_>>,
    init_flat_len: usize,
    is_kotlinlang: bool,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    let Some(init) = init_child else { return 0 };
    if is_kotlinlang {
        let is_lambda = find_first_lambda(init).is_some();
        if !is_lambda {
            return init_flat_len;
        }
        let is_chained_call = crate::lower::chain::has_call_in_receiver_chain(&init)
            || matches!(init.kind(), "navigation_expression");
        let is_scoping_or_lambda = crate::lower::scoping::is_scoping_chain_or_lambda(&init);
        if !is_scoping_or_lambda && is_chained_call {
            return init_flat_len;
        }
        return crate::lower::expression_body::expression_first_line_len(&init, 0, ctx);
    }
    if let Some(call) = find_opening_call(init)
        && call_has_multiline_args(&call, ctx)
    {
        if let Some(len) = find_call_opening_paren_len(call, init, ctx) {
            return len;
        }
        if let Some(lambda) = find_first_lambda(call) {
            return lambda.start_byte().saturating_sub(init.start_byte()) + 1;
        }
    }
    if let Some(lambda) = find_first_lambda(init) {
        return lambda.start_byte().saturating_sub(init.start_byte()) + 1;
    }
    init_flat_len
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn property_first_line_fits(
    node: &Node<'_>,
    modifier_len: usize,
    name_len: usize,
    type_len: usize,
    op: &str,
    init_child: Option<Node<'_>>,
    init_flat_len: usize,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let indent = ctx.compute_target_indent(node);
    let op_len = op.len() + 1;
    let is_kotlinlang = ctx.options.block_indent == 4;
    let init_len = property_init_first_line_len(init_child, init_flat_len, is_kotlinlang, ctx);
    indent + modifier_len + name_len + type_len + op_len + init_len <= ctx.options.max_width
}
