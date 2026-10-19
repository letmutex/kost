use tree_sitter::Node;

use crate::doc::Doc;
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

pub(crate) fn call_has_args(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut suffix_cursor = child.walk();
            for suffix_child in child.children(&mut suffix_cursor) {
                if suffix_child.kind() == "value_arguments" {
                    return suffix_child
                        .children(&mut suffix_child.walk())
                        .any(|c| c.kind() == "value_argument");
                }
            }
        }
    }
    false
}

pub(crate) fn call_has_multiline_args(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut suffix_cursor = child.walk();
            for suffix_child in child.children(&mut suffix_cursor) {
                if suffix_child.kind() == "value_arguments" {
                    return suffix_child.start_position().row != suffix_child.end_position().row
                        || suffix_child.byte_range().len() > ctx.options.max_width
                        || crate::lower::lists::has_trailing_comma(&suffix_child);
                }
            }
        }
    }
    false
}

pub(crate) fn find_innermost_call<'a>(node: Node<'a>) -> Option<Node<'a>> {
    let mut curr = node;
    let mut root_call = None;
    while curr.kind() == "navigation_expression" || curr.kind() == "call_expression" {
        if curr.kind() == "call_expression" {
            root_call = Some(curr);
        }
        if let Some(first) = curr.child(0)
            && (first.kind() == "navigation_expression" || first.kind() == "call_expression")
        {
            curr = first;
            continue;
        }
        break;
    }
    root_call
}

pub(crate) fn find_opening_call<'a>(node: Node<'a>) -> Option<Node<'a>> {
    find_innermost_call(node)
}

pub(crate) fn build_property_init_section<'a>(
    init_docs: Vec<Doc<'a>>,
    init_child: Option<Node<'a>>,
    trailing_comments: Vec<PropertyComment<'a>>,
    is_destructuring: bool,
    ctx: &LowerContext<'a, '_>,
) -> (Doc<'a>, bool) {
    let init_doc = Doc::Concat(init_docs);
    let is_kotlinlang = ctx.options.block_indent == 4;
    let flat_init = init_child.is_some_and(|r| {
        let opening = find_opening_call(r);
        crate::lower::scoping::is_scoping_chain_or_lambda(&r)
            || ((is_kotlinlang || is_destructuring)
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

pub(crate) fn find_first_lambda<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if matches!(node.kind(), "lambda_literal" | "annotated_lambda") {
        return Some(node);
    }
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        if ch.kind() == "value_arguments" {
            continue;
        }
        if let Some(l) = find_first_lambda(ch) {
            return Some(l);
        }
    }
    None
}

pub(crate) fn find_call_opening_paren_len(
    call: Node<'_>,
    base: Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> Option<usize> {
    let mut cursor = call.walk();
    for ch in call.children(&mut cursor) {
        if ch.kind() == "call_suffix" {
            let mut sc = ch.walk();
            for sub in ch.children(&mut sc) {
                if sub.kind() == "value_arguments" {
                    let has_args = sub
                        .children(&mut sub.walk())
                        .any(|c| c.kind() == "value_argument");
                    if has_args {
                        if base.start_position().row != sub.start_position().row {
                            let first_line = ctx.node_text(&base).lines().next().unwrap_or("");
                            return Some(first_line.len());
                        }
                        return Some(sub.start_byte().saturating_sub(base.start_byte()) + 1);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn property_init_first_line_len(
    init_child: Option<Node<'_>>,
    init_flat_len: usize,
    is_kotlinlang: bool,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    let Some(init) = init_child else { return 0 };
    if let Some(call) = find_opening_call(init)
        && (is_kotlinlang || call_has_args(&call))
        && let Some(len) = find_call_opening_paren_len(call, init, ctx)
    {
        return len;
    }
    if let Some(lambda) = find_first_lambda(init) {
        if init.start_position().row != lambda.start_position().row {
            let first_line = ctx.node_text(&init).lines().next().unwrap_or("");
            return first_line.len();
        }
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
    let op_len = op.len() + 2;
    let is_kotlinlang = ctx.options.block_indent == 4;
    let init_len = property_init_first_line_len(init_child, init_flat_len, is_kotlinlang, ctx);
    indent + modifier_len + name_len + type_len + op_len + init_len <= ctx.options.max_width
}
