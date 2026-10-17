use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::{lower_comment, lower_node};

pub fn lower_directly_assignable_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_node(child, ctx));
    }
    Doc::Group(Box::new(Doc::Concat(docs)))
}

pub(crate) fn has_trailing_lambda_child(node: &Node<'_>) -> bool {
    let kind = node.kind();
    if matches!(kind, "lambda_literal" | "annotated_lambda") {
        return true;
    }
    if kind == "call_expression" {
        let mut c = node.walk();
        return node.children(&mut c).any(|child| {
            if child.kind() == "call_suffix" {
                let mut c2 = child.walk();
                child
                    .children(&mut c2)
                    .any(|ch| matches!(ch.kind(), "lambda_literal" | "annotated_lambda"))
            } else if child.kind() == "navigation_expression" {
                has_trailing_lambda_child(&child)
            } else {
                matches!(child.kind(), "lambda_literal" | "annotated_lambda")
            }
        });
    }
    if kind == "navigation_expression" {
        let mut c = node.walk();
        return node
            .children(&mut c)
            .any(|child| has_trailing_lambda_child(&child));
    }
    false
}

#[derive(Default)]
struct AssignmentParts<'a> {
    lhs: Option<Doc<'a>>,
    op: Option<&'a str>,
    rhs: Option<Doc<'a>>,
    rhs_node: Option<Node<'a>>,
    comment_doc: Option<Doc<'a>>,
    comment_has_nl: bool,
}

fn parse_assignment_parts<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> AssignmentParts<'a> {
    let mut parts = AssignmentParts::default();
    let mut op_node = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(kind, "=" | "+=" | "-=" | "*=" | "/=" | "%=") {
            parts.op = Some(ctx.node_text(&child));
            op_node = Some(child);
        } else if parts.op.is_none() {
            parts.lhs = Some(lower_node(child, ctx));
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if parts.rhs_node.is_none() && parts.comment_doc.is_none() {
                let has_nl = op_node.is_some_and(|o| ctx.has_newline_between(&o, &child));
                parts.comment_has_nl = has_nl;
                let c_doc = lower_comment(child, ctx);
                let clean_doc = match c_doc {
                    Doc::Concat(mut items) => {
                        if items.last().is_some_and(|d| matches!(d, Doc::HardLine)) {
                            items.pop();
                        }
                        Doc::Concat(items)
                    }
                    c => c,
                };
                parts.comment_doc = Some(clean_doc);
            }
        } else {
            if parts.rhs_node.is_none() {
                parts.rhs_node = Some(child);
            }
            parts.rhs = Some(lower_node(child, ctx));
        }
    }
    parts
}

pub fn lower_assignment<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let parts = parse_assignment_parts(node, ctx);
    let lhs_doc = parts.lhs.unwrap_or_else(Doc::empty);
    let op_str = parts.op.unwrap_or("=");
    let rhs_doc = parts.rhs.unwrap_or_else(Doc::empty);

    if let Some(c_doc) = parts.comment_doc {
        let sep = if parts.comment_has_nl {
            Doc::HardLine
        } else {
            Doc::space()
        };
        return Doc::Concat(vec![
            lhs_doc,
            Doc::space(),
            Doc::text(op_str),
            sep,
            c_doc,
            Doc::space(),
            rhs_doc,
        ]);
    }

    let is_block_like = parts.rhs_node.is_some_and(|r| {
        has_trailing_lambda_child(&r)
            || (ctx.options.block_indent == 4
                && matches!(r.kind(), "call_expression" | "navigation_expression")
                && ctx.node_text(&r).contains('\n'))
    });
    if is_block_like && let Some(r_node) = parts.rhs_node {
        let rhs_raw = ctx.node_text(&r_node);
        let first_line = rhs_raw.lines().next().unwrap_or("");
        let lhs_len = node.child(0).map(|c| ctx.node_text(&c).len()).unwrap_or(0);
        let total = lhs_len + 1 + op_str.len() + 1 + first_line.len();
        if total + ctx.options.block_indent <= ctx.options.max_width {
            return Doc::Concat(vec![
                lhs_doc,
                Doc::space(),
                Doc::text(op_str),
                Doc::space(),
                rhs_doc,
            ]);
        }
    }

    Doc::Concat(vec![
        lhs_doc,
        Doc::space(),
        Doc::text(op_str),
        Doc::Group(Box::new(Doc::if_break(
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, rhs_doc.clone()])),
            Doc::Concat(vec![Doc::space(), rhs_doc]),
        ))),
    ])
}
