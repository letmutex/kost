use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::BlockItem;
use crate::lower::context::LowerContext;

pub fn is_bodiless_loop(node: &Node<'_>) -> bool {
    matches!(node.kind(), "for_statement" | "while_statement")
        && !node
            .children(&mut node.walk())
            .any(|c| matches!(c.kind(), "control_structure_body" | "block" | "{" | ";"))
}

pub fn try_attach_loop_body<'a>(
    items: &mut [BlockItem<'a>],
    child: Node<'a>,
    prev_child: &mut Option<Node<'a>>,
    last_trailing_comment: &mut Option<Node<'a>>,
    has_semi_before: bool,
    child_is_comment: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> bool {
    if !has_semi_before
        && !child_is_comment
        && prev_child.as_ref().is_some_and(is_bodiless_loop)
        && let Some(last_item) = items.last_mut()
    {
        let prev_doc = std::mem::replace(&mut last_item.doc, Doc::empty());
        *last_trailing_comment = None;
        last_item.doc = Doc::group(Doc::concat(vec![
            prev_doc,
            Doc::indent(Doc::concat(vec![Doc::Line, lower_fn(child, ctx)])),
        ]));
        *prev_child = Some(child);
        return true;
    }
    false
}

fn is_braced_body(node: &Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "block" || kind == "function_body" {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| c.kind() == "{" || c.kind() == "block")
}

pub fn lower_for_statement<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut param_node = None;
    let mut range_node = None;
    let mut body_node = None;
    let mut seen_in = false;
    let mut in_parens = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "for" => {}
            "(" => in_parens = true,
            ")" => in_parens = false,
            "in" => seen_in = true,
            _ => {
                if in_parens {
                    if !seen_in {
                        param_node = Some(child);
                    } else if range_node.is_none() {
                        range_node = Some(child);
                    }
                } else if body_node.is_none() {
                    body_node = Some(child);
                    break;
                }
            }
        }
    }

    let mut header = Vec::new();
    header.push(Doc::text("for ("));
    if let Some(param) = param_node {
        header.push(lower_fn(param, ctx));
    }
    header.push(Doc::text(" in"));
    if let Some(range) = range_node {
        header.push(Doc::group(Doc::continuation_indent(Doc::concat(vec![
            Doc::Line,
            lower_fn(range, ctx),
        ]))));
    }
    header.push(Doc::text(")"));

    let header_doc = Doc::concat(header);
    let Some(body) = body_node else {
        return header_doc;
    };

    let is_braced = is_braced_body(&body);
    if is_braced {
        Doc::concat(vec![header_doc, Doc::space(), lower_fn(body, ctx)])
    } else {
        Doc::group(Doc::concat(vec![
            header_doc,
            Doc::indent(Doc::concat(vec![Doc::Line, lower_fn(body, ctx)])),
        ]))
    }
}

pub fn lower_while_statement<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut cond_node = None;
    let mut body_node = None;
    let mut comments = Vec::new();
    let mut has_semi = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "while" | "(" | ")" => {}
            ";" => has_semi = true,
            "comment" | "line_comment" | "multiline_comment" => {
                comments.push(lower_fn(child, ctx));
            }
            _ => {
                if cond_node.is_none() {
                    cond_node = Some(child);
                } else if body_node.is_none() {
                    body_node = Some(child);
                }
            }
        }
    }

    let cond_doc = cond_node
        .map(|c| lower_fn(c, ctx))
        .unwrap_or_else(Doc::empty);
    let while_head = if ctx.style.soft_break_inside_condition_parens {
        Doc::group(Doc::concat(vec![
            Doc::text("while ("),
            Doc::continuation_indent(Doc::concat(vec![Doc::SoftLine, cond_doc])),
            Doc::SoftLine,
            Doc::text(")"),
        ]))
    } else {
        Doc::concat(vec![Doc::text("while ("), cond_doc, Doc::text(")")])
    };

    let mut docs = vec![while_head];

    if let Some(body) = body_node {
        if is_braced_body(&body) {
            docs.push(Doc::space());
            docs.push(lower_fn(body, ctx));
        } else {
            docs.push(Doc::indent(Doc::concat(vec![
                Doc::Line,
                lower_fn(body, ctx),
            ])));
        }
    } else if has_semi {
        if comments.is_empty() {
            docs.push(Doc::space());
            docs.push(Doc::text(";"));
        } else {
            for c in comments {
                docs.push(Doc::HardLine);
                docs.push(c);
            }
            docs.push(Doc::HardLine);
            docs.push(Doc::text(";"));
        }
    }

    Doc::group(Doc::concat(docs))
}

pub fn lower_do_while_statement<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut body_node = None;
    let mut cond_node = None;
    let mut seen_while = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "do" | "(" | ")" => {}
            "while" => seen_while = true,
            _ => {
                if !seen_while {
                    body_node = Some(child);
                } else if cond_node.is_none() {
                    cond_node = Some(child);
                }
            }
        }
    }

    let mut docs = Vec::new();
    docs.push(Doc::text("do "));
    if let Some(body) = body_node {
        docs.push(lower_fn(body, ctx));
        docs.push(Doc::space());
    }
    docs.push(Doc::text("while ("));
    if let Some(cond) = cond_node {
        docs.push(lower_fn(cond, ctx));
    }
    docs.push(Doc::text(")"));

    Doc::concat(docs)
}
