use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;
use crate::lower::scoping::{is_pure_lambda_or_labeled, is_scoping_chain_or_lambda};

fn has_complex_or_control_receiver(node: &Node<'_>) -> bool {
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if matches!(
            n.kind(),
            "elvis_expression"
                | "binary_expression"
                | "if_expression"
                | "when_expression"
                | "try_expression"
        ) {
            return true;
        }
        curr = n.child(0);
    }
    false
}

fn is_pascal_case_call(node: &Node<'_>, source: &str) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let Some(callee) = node.child(0) else {
        return false;
    };
    callee.kind() == "simple_identifier"
        && source
            .get(callee.start_byte()..callee.end_byte())
            .is_some_and(|n| n.starts_with(|c: char| c.is_ascii_uppercase()))
}

pub(crate) fn chain_root_element<'a>(node: Node<'a>) -> Node<'a> {
    let mut curr = node;
    loop {
        let callee = if curr.kind() == "call_expression" {
            curr.child(0)
        } else if curr.kind() == "navigation_expression" {
            Some(curr)
        } else {
            None
        };
        let Some(nav) = callee.filter(|c| c.kind() == "navigation_expression") else {
            break;
        };
        let Some(raw_receiver) = nav.child(0) else {
            break;
        };
        let receiver = if raw_receiver.kind() == "postfix_expression" {
            raw_receiver.child(0).unwrap_or(raw_receiver)
        } else {
            raw_receiver
        };
        if matches!(receiver.kind(), "call_expression" | "navigation_expression") {
            curr = receiver;
            continue;
        }
        break;
    }
    curr
}

pub(crate) fn find_lambda_first_line_end_byte(lambda: Node<'_>) -> usize {
    let mut cursor = lambda.walk();
    for ch in lambda.children(&mut cursor) {
        if ch.kind() == "->" {
            return ch.end_byte();
        }
        if ch.kind() == "lambda_literal" {
            return find_lambda_first_line_end_byte(ch);
        }
    }
    for ch in lambda.children(&mut cursor) {
        if ch.kind() == "{" {
            return ch.end_byte();
        }
    }
    lambda.end_byte()
}

pub(crate) fn collapsed_text_len(text: &str) -> usize {
    let mut len = 0;
    let mut in_whitespace = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            in_whitespace = true;
        } else {
            if in_whitespace {
                if len > 0 && !matches!(ch, '.' | '(' | ')' | '[' | ']' | ',' | '?' | ':') {
                    len += 1;
                }
                in_whitespace = false;
            }
            len += 1;
        }
    }
    len
}

pub(crate) fn expression_first_line_len(
    node: &Node<'_>,
    _eq_col: usize,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    if let Some(call) = crate::lower::delegates::find_first_call_with_multiline_args(*node, ctx)
        && let Some(len) = crate::lower::delegates::find_call_opening_paren_len(call, *node, ctx)
    {
        return len;
    }
    if let Some(lambda) = crate::lower::delegates::find_first_lambda(*node)
        && crate::lower::chain::count_chain_lambdas(node) <= 1
    {
        let len = find_lambda_first_line_end_byte(lambda).saturating_sub(node.start_byte());
        if len <= ctx.options.max_width {
            return len;
        }
    }
    let root = chain_root_element(*node);
    if let Some(call) = crate::lower::delegates::find_innermost_call(root)
        && (crate::lower::delegates::call_has_multiline_args(&call, ctx)
            || ctx.compute_target_indent(node)
                + ctx.options.continuation_indent
                + call.byte_range().len()
                > ctx.options.max_width)
        && let Some(len) = crate::lower::delegates::find_call_opening_paren_len(call, root, ctx)
    {
        return len;
    }
    if let Some(lambda) = crate::lower::delegates::find_first_lambda(root) {
        return find_lambda_first_line_end_byte(lambda).saturating_sub(root.start_byte());
    }
    if matches!(root.kind(), "string_literal" | "multiline_string_literal") {
        return ctx.node_text(&root).lines().next().unwrap_or("").len();
    }
    collapsed_text_len(ctx.node_text(&root))
}

fn append_comments_after_expr<'a>(
    expr_doc: Doc<'a>,
    expr_child: Option<Node<'a>>,
    comments: Vec<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if comments.is_empty() {
        return expr_doc;
    }
    let mut after_parts = vec![expr_doc];
    let mut prev = expr_child;
    for c in comments {
        if prev.is_some_and(|p| p.end_position().row == c.start_position().row) {
            after_parts.push(Doc::space());
        } else {
            after_parts.push(Doc::HardLine);
        }
        after_parts.push(lower_node(c, ctx));
        prev = Some(c);
    }
    Doc::Concat(after_parts)
}

fn format_comments_before_expr<'a>(
    comments: Vec<Node<'a>>,
    expr_doc: Doc<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut inner = Vec::new();
    for c in comments {
        inner.push(Doc::HardLine);
        inner.push(lower_node(c, ctx));
    }
    if !inner.last().is_some_and(|d| d.ends_with_newline()) {
        inner.push(Doc::HardLine);
    }
    inner.push(expr_doc);
    Doc::Concat(vec![
        Doc::text("="),
        Doc::continuation_indent(Doc::Concat(inner)),
    ])
}

pub fn format_expression_body<'a>(
    node: Node<'a>,
    expr_child: Option<Node<'a>>,
    comments_before_expr: Vec<Node<'a>>,
    comments_after_expr: Vec<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    header_col: Option<usize>,
    is_multiline_header: bool,
) -> Doc<'a> {
    let raw_expr_doc = expr_child
        .map(|c| lower_node(c, ctx))
        .unwrap_or_else(Doc::empty);
    let expr_doc = append_comments_after_expr(raw_expr_doc, expr_child, comments_after_expr, ctx);
    if !comments_before_expr.is_empty() {
        return format_comments_before_expr(comments_before_expr, expr_doc, ctx);
    }

    if let Some(expr_node) = expr_child {
        if is_pure_lambda_or_labeled(&expr_node) {
            return Doc::Concat(vec![Doc::text("="), Doc::space(), expr_doc]);
        }
        let eq_col = header_col.unwrap_or_else(|| ctx.compute_target_indent(&node));
        let is_chain_or_multiline_call = expr_node.kind() == "navigation_expression"
            || expr_node
                .child(0)
                .is_some_and(|c| c.kind() == "navigation_expression")
            || crate::lower::delegates::find_innermost_call(expr_node).is_some_and(|innermost| {
                let will_wrap = crate::lower::delegates::call_has_multiline_args(&innermost, ctx)
                    || ctx.compute_target_indent(&node)
                        + ctx.options.continuation_indent
                        + innermost.byte_range().len()
                        > ctx.options.max_width;
                will_wrap
                    && crate::lower::delegates::find_call_opening_paren_len(
                        innermost, expr_node, ctx,
                    )
                    .is_some()
            });
        let is_block_like = !is_pascal_case_call(&expr_node, ctx.source)
            && (expr_node.kind() == "call_expression"
                || expr_node.kind() == "navigation_expression")
            && (crate::lower::assignment::has_trailing_lambda_child(&expr_node)
                || is_scoping_chain_or_lambda(&expr_node)
                || (!is_multiline_header
                    && ctx.options.block_indent == 4
                    && is_chain_or_multiline_call));

        if is_block_like && !has_complex_or_control_receiver(&expr_node) {
            let first_line_len = expression_first_line_len(&expr_node, eq_col, ctx);
            let total = eq_col + 3 + first_line_len;
            if total <= ctx.options.max_width {
                return Doc::Concat(vec![Doc::text("="), Doc::space(), expr_doc]);
            }
        }
    }
    Doc::Concat(vec![
        Doc::text("="),
        Doc::if_break(
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, expr_doc.clone()])),
            Doc::Concat(vec![Doc::space(), expr_doc]),
        ),
    ])
}
