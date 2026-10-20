use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

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
            } else {
                matches!(child.kind(), "lambda_literal" | "annotated_lambda")
            }
        });
    }
    false
}

#[derive(Default)]
struct AssignmentParts<'a> {
    lhs: Option<Doc<'a>>,
    op: Option<&'a str>,
    op_node: Option<Node<'a>>,
    rhs: Option<Doc<'a>>,
    rhs_node: Option<Node<'a>>,
    comments_before_rhs: Vec<Node<'a>>,
    comments_after_rhs: Vec<Node<'a>>,
}

fn parse_assignment_parts<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> AssignmentParts<'a> {
    let mut parts = AssignmentParts::default();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(kind, "=" | "+=" | "-=" | "*=" | "/=" | "%=") {
            parts.op = Some(ctx.node_text(&child));
            parts.op_node = Some(child);
        } else if parts.op.is_none() {
            parts.lhs = Some(lower_node(child, ctx));
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if parts.rhs_node.is_none() {
                parts.comments_before_rhs.push(child);
            } else {
                parts.comments_after_rhs.push(child);
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

fn append_post_rhs_comments<'a>(
    rhs_doc: Doc<'a>,
    parts: &AssignmentParts<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if parts.comments_after_rhs.is_empty() {
        return rhs_doc;
    }
    let mut after_parts = vec![rhs_doc];
    let mut prev = parts.rhs_node;
    for c in &parts.comments_after_rhs {
        if prev.is_some_and(|p| p.end_position().row == c.start_position().row) {
            after_parts.push(Doc::space());
        } else {
            after_parts.push(Doc::HardLine);
        }
        after_parts.push(lower_node(*c, ctx));
        prev = Some(*c);
    }
    Doc::Concat(after_parts)
}

fn compute_assignment_rhs_first_line_len(r_node: Node<'_>, ctx: &LowerContext<'_, '_>) -> usize {
    let is_chained_call = crate::lower::chain::has_call_in_receiver_chain(&r_node)
        || matches!(r_node.kind(), "navigation_expression");
    if !is_chained_call
        && let Some(call) = crate::lower::delegates::find_opening_call(r_node)
        && crate::lower::delegates::call_has_multiline_args(&call, ctx)
        && let Some(len) = crate::lower::delegates::find_call_opening_paren_len(call, r_node, ctx)
    {
        len
    } else if let Some(lambda) = crate::lower::delegates::find_first_lambda(r_node) {
        let end_byte = crate::lower::expression_body::find_lambda_first_line_end_byte(lambda);
        let text = ctx.source.get(r_node.start_byte()..end_byte).unwrap_or("");
        crate::lower::expression_body::collapsed_text_len(text)
    } else {
        crate::lower::expression_body::collapsed_text_len(ctx.node_text(&r_node))
    }
}

fn assignment_rhs_first_line_fits(
    node: &Node<'_>,
    r_node: Node<'_>,
    op_str: &str,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let is_top_level = ctx.compute_target_indent(node) == 0;
    let flat_rhs = has_trailing_lambda_child(&r_node)
        || crate::lower::scoping::is_scoping_chain_or_lambda(&r_node)
        || (ctx.options.block_indent == 4
            && is_top_level
            && matches!(r_node.kind(), "call_expression" | "navigation_expression")
            && crate::lower::delegates::find_innermost_call(r_node).is_some_and(|innermost| {
                let opening_has_receiver = innermost
                    .child(0)
                    .is_some_and(|callee| callee.kind() == "navigation_expression");
                !opening_has_receiver
                    && crate::lower::delegates::find_call_opening_paren_len(innermost, r_node, ctx)
                        .is_some()
            }));
    if !flat_rhs {
        return false;
    }
    let first_line_len = compute_assignment_rhs_first_line_len(r_node, ctx);
    let indent = ctx.compute_target_indent(node);
    let lhs_len = node.child(0).map(|c| ctx.node_text(&c).len()).unwrap_or(0);
    indent + lhs_len + 1 + op_str.len() + 1 + first_line_len <= ctx.options.max_width
}

pub fn lower_assignment<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut parts = parse_assignment_parts(node, ctx);
    let lhs_doc = parts.lhs.take().unwrap_or_else(Doc::empty);
    let op_str = parts.op.unwrap_or("=");
    let rhs_doc =
        append_post_rhs_comments(parts.rhs.take().unwrap_or_else(Doc::empty), &parts, ctx);

    if !parts.comments_before_rhs.is_empty() {
        return format_assignment_with_preceding_comments(
            node, lhs_doc, op_str, rhs_doc, &parts, ctx,
        );
    }

    if parts
        .rhs_node
        .is_some_and(|r| assignment_rhs_first_line_fits(&node, r, op_str, ctx))
    {
        return Doc::Concat(vec![
            lhs_doc,
            Doc::space(),
            Doc::text(op_str),
            Doc::space(),
            rhs_doc,
        ]);
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

fn is_explicit_backing_field_assignment(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() == "property_declaration" {
        return true;
    }
    if matches!(parent.kind(), "class_body" | "source_file") {
        let is_lhs_field = node
            .child(0)
            .is_some_and(|c| ctx.node_text(&c).trim() == "field");
        let has_prev_property = node
            .prev_named_sibling()
            .is_some_and(|s| s.kind() == "property_declaration");
        return is_lhs_field && has_prev_property;
    }
    false
}

fn format_assignment_with_preceding_comments<'a>(
    node: Node<'a>,
    lhs_doc: Doc<'a>,
    op_str: &'a str,
    rhs_doc: Doc<'a>,
    parts: &AssignmentParts<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let last_comment = *parts.comments_before_rhs.last().unwrap();
    let has_newline_after_last = parts
        .rhs_node
        .is_some_and(|r| ctx.has_newline_between(&last_comment, &r));
    let is_last_line_comment = last_comment.kind() == "line_comment";

    let is_field_accessor = is_explicit_backing_field_assignment(&node, ctx);

    let mut inner = Vec::new();
    for (i, c) in parts.comments_before_rhs.iter().enumerate() {
        let has_newline_before = if i > 0 {
            ctx.has_newline_between(&parts.comments_before_rhs[i - 1], c)
        } else {
            parts
                .op_node
                .is_some_and(|op| ctx.has_newline_between(&op, c))
        };
        if has_newline_before {
            inner.push(Doc::HardLine);
        } else {
            inner.push(Doc::space());
        }
        inner.push(lower_node(*c, ctx));
    }

    if is_last_line_comment || has_newline_after_last {
        if !inner.last().is_some_and(|d| d.ends_with_newline()) {
            inner.push(Doc::HardLine);
        }
    } else {
        inner.push(Doc::space());
    }
    inner.push(rhs_doc);

    let formatted_inner = if is_field_accessor {
        Doc::Concat(inner)
    } else {
        Doc::continuation_indent(Doc::Concat(inner))
    };

    Doc::Concat(vec![
        lhs_doc,
        Doc::space(),
        Doc::text(op_str),
        formatted_inner,
    ])
}
