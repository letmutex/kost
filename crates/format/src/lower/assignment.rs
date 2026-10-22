use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{format_assign_like_ext, format_assign_with_comments};
use crate::lower::context::LowerContext;

pub fn lower_destructuring_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut modifier_docs = Vec::new();
    let mut lhs_doc = None;
    let mut rhs_node = None;
    let mut is_after_equals = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "modifiers" | "annotation" => {
                modifier_docs.push(lower_fn(child, ctx));
            }
            "=" => is_after_equals = true,
            _ => {
                if !is_after_equals {
                    lhs_doc = Some(lower_fn(child, ctx));
                } else if rhs_node.is_none() {
                    rhs_node = Some(child);
                }
            }
        }
    }

    let lhs = lhs_doc.unwrap_or(Doc::empty());
    let assign_doc = if let Some(rhs) = rhs_node {
        format_assign_like_ext(lhs, "=", rhs, lower_fn(rhs, ctx), true, ctx)
    } else {
        lhs
    };
    if modifier_docs.is_empty() {
        assign_doc
    } else {
        modifier_docs.push(assign_doc);
        Doc::concat(modifier_docs)
    }
}

pub fn lower_assignment_with_prefix<'a>(
    node: Node<'a>,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut lhs_node = None;
    let mut op_str = "=";
    let mut op_node = None;
    let mut rhs_node = None;
    let mut comments_before_rhs = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(kind, "=" | "+=" | "-=" | "*=" | "/=" | "%=") {
            op_str = ctx.node_text(&child);
            op_node = Some(child);
        } else if lhs_node.is_none() {
            lhs_node = Some(child);
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if rhs_node.is_none() {
                comments_before_rhs.push(child);
            }
        } else if rhs_node.is_none() {
            rhs_node = Some(child);
        }
    }

    let (Some(lhs), Some(rhs)) = (lhs_node, rhs_node) else {
        return Doc::text(ctx.node_text(&node));
    };

    let mut lhs_doc = lower_fn(lhs, ctx);
    if let Some(prefix) = prefix_doc {
        lhs_doc = Doc::group(Doc::concat(vec![prefix, Doc::Line, lhs_doc]));
    }
    let rhs_doc = lower_fn(rhs, ctx);
    format_assign_with_comments(
        lhs_doc,
        op_str,
        op_node,
        &comments_before_rhs,
        rhs,
        rhs_doc,
        ctx,
        lower_fn,
    )
}

pub fn lower_assignment<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    lower_assignment_with_prefix(node, None, ctx, lower_fn)
}
