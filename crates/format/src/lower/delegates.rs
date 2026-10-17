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
    trailing_comment: &mut Option<PropertyComment<'a>>,
) {
    let mut by_cursor = child.walk();
    let mut by_node = None;
    for c in child.children(&mut by_cursor) {
        let kind = c.kind();
        if kind == "by" {
            by_node = Some(c);
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if init_child.is_none() && trailing_comment.is_none() {
                *trailing_comment = Some(parse_comment_before_init(c, by_node, ctx));
            }
        } else if init_child.is_none() {
            *init_child = Some(c);
            init_docs.push(lower_node(c, ctx));
        }
    }
}

pub(crate) fn call_has_multiline_args(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut sc = child.walk();
            for sch in child.children(&mut sc) {
                if sch.kind() == "value_arguments"
                    && sch.start_position().row != sch.end_position().row
                {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn call_or_chain_has_multiline_args(node: &Node<'_>) -> bool {
    let mut curr = *node;
    while curr.kind() == "navigation_expression" || curr.kind() == "call_expression" {
        if call_has_multiline_args(&curr) {
            return true;
        }
        if let Some(first) = curr.child(0) {
            curr = first;
        } else {
            break;
        }
    }
    false
}

pub(crate) fn build_property_init_section<'a>(
    init_docs: Vec<Doc<'a>>,
    init_child: Option<Node<'a>>,
    trailing_comment: Option<PropertyComment<'a>>,
    is_kotlinlang: bool,
    flat_call: bool,
) -> (Doc<'a>, bool) {
    let init_doc = Doc::Concat(init_docs);
    let flat_init = init_child.is_some_and(|r| {
        crate::lower::scoping::is_scoping_chain_or_lambda(&r)
            || (is_kotlinlang && call_or_chain_has_multiline_args(&r))
            || flat_call
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

    let doc = if let Some(comment) = trailing_comment {
        let comment_text = match comment.doc {
            Doc::Concat(mut items) => {
                if items.last().is_some_and(|d| matches!(d, Doc::HardLine)) {
                    items.pop();
                }
                Doc::Concat(items)
            }
            c => c,
        };
        if comment.is_line_comment {
            if comment.has_newline_before {
                Doc::continuation_indent(Doc::Concat(vec![
                    Doc::HardLine,
                    comment_text,
                    Doc::HardLine,
                    init_doc,
                ]))
            } else {
                Doc::Concat(vec![
                    Doc::space(),
                    comment_text,
                    Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, init_doc])),
                ])
            }
        } else if comment.has_newline_before {
            Doc::Concat(vec![
                Doc::indent(Doc::Concat(vec![Doc::HardLine, comment_text, Doc::space()])),
                init_doc,
            ])
        } else {
            Doc::Concat(vec![Doc::space(), comment_text, Doc::space(), init_doc])
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
