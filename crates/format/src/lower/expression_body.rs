use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;
use crate::lower::scoping::{is_pure_lambda_or_labeled, is_scoping_chain_or_lambda};

fn call_has_complex_receiver(call: &Node<'_>) -> bool {
    if let Some(callee) = call.child(0)
        && callee.kind() == "navigation_expression"
        && let Some(receiver) = callee.child(0)
    {
        if matches!(
            receiver.kind(),
            "parenthesized_expression"
                | "navigation_expression"
                | "indexing_expression"
                | "collection_literal"
        ) {
            return true;
        }
        if receiver.kind() == "call_expression"
            && !crate::lower::assignment::has_trailing_lambda_child(&receiver)
        {
            return true;
        }
    }
    false
}

fn has_complex_or_control_receiver(node: &Node<'_>) -> bool {
    if node.kind() == "call_expression"
        && crate::lower::assignment::has_trailing_lambda_child(node)
        && (call_has_complex_receiver(node) || crate::lower::call_args::call_has_args(node))
    {
        return true;
    }
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

pub(crate) use crate::lower::call_args::find_lambda_first_line_end_byte;

pub(crate) fn collapsed_text_len(text: &str) -> usize {
    let mut len = 0;
    let mut in_whitespace = false;
    let mut in_string = false;
    let mut escape = false;
    for ch in text.chars() {
        if in_string {
            len += 1;
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            if in_whitespace {
                if len > 0 {
                    len += 1;
                }
                in_whitespace = false;
            }
            len += 1;
            in_string = true;
        } else if ch.is_whitespace() {
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
    if let Some(call) = crate::lower::delegates::find_opening_call(*node)
        && crate::lower::delegates::call_has_multiline_args(&call, ctx)
        && let Some(len) = crate::lower::delegates::find_call_opening_paren_len(call, *node, ctx)
    {
        return len;
    }
    if let Some(lambda) = crate::lower::delegates::find_first_lambda(*node)
        && crate::lower::chain::count_chain_lambdas(node) <= 1
    {
        let end_byte = find_lambda_first_line_end_byte(lambda);
        let text = ctx.source.get(node.start_byte()..end_byte).unwrap_or("");
        let len = collapsed_text_len(text);
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
        let end_byte = find_lambda_first_line_end_byte(lambda);
        let text = ctx.source.get(root.start_byte()..end_byte).unwrap_or("");
        return collapsed_text_len(text);
    }
    if matches!(root.kind(), "string_literal" | "multiline_string_literal") {
        return ctx.node_text(&root).lines().next().unwrap_or("").len();
    }
    if crate::lower::chain::has_call_in_receiver_chain(node)
        || node.kind() == "navigation_expression"
    {
        return collapsed_text_len(ctx.node_text(node));
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

fn find_expression_body_trailing_comment<'a>(
    node: &Node<'a>,
    comments_after_expr: &[Node<'a>],
    ctx: &LowerContext<'a, '_>,
) -> Option<Node<'a>> {
    comments_after_expr.first().copied().or_else(|| {
        node.parent().and_then(|p| p.next_sibling()).filter(|s| {
            matches!(s.kind(), "comment" | "line_comment" | "multiline_comment")
                && !ctx.has_newline_between(node, s)
        })
    })
}

fn is_block_like_expression(
    expr_node: &Node<'_>,
    node: &Node<'_>,
    is_multiline_header: bool,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    if is_pascal_case_call(expr_node, ctx.source) {
        return false;
    }
    if !matches!(
        expr_node.kind(),
        "call_expression" | "navigation_expression"
    ) {
        return false;
    }
    let has_trailing_lambda = crate::lower::assignment::has_trailing_lambda_child(expr_node);
    let scoping = is_scoping_chain_or_lambda(expr_node);
    if has_trailing_lambda || scoping {
        return true;
    }
    if !is_multiline_header && ctx.options.block_indent == 4 {
        let is_top_level = node
            .parent()
            .is_some_and(|decl| ctx.compute_target_indent(&decl) == 0);
        let has_lambda = crate::lower::delegates::find_first_lambda(*expr_node).is_some();
        return crate::lower::delegates::find_opening_call(*expr_node).is_some_and(|opening| {
            let opening_has_receiver = opening
                .child(0)
                .is_some_and(|callee| callee.kind() == "navigation_expression");
            let will_wrap = crate::lower::delegates::call_has_multiline_args(&opening, ctx);
            is_top_level
                && !has_lambda
                && !opening_has_receiver
                && will_wrap
                && crate::lower::delegates::find_call_opening_paren_len(opening, *expr_node, ctx)
                    .is_some()
        });
    }
    false
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
    let trailing_comment = find_expression_body_trailing_comment(&node, &comments_after_expr, ctx);
    let trailing_comment_len = trailing_comment
        .map(|c| 1 + ctx.node_text(&c).len())
        .unwrap_or(0);
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
        let is_block_like = is_block_like_expression(&expr_node, &node, is_multiline_header, ctx);

        if is_block_like && !has_complex_or_control_receiver(&expr_node) {
            let first_line_len = expression_first_line_len(&expr_node, eq_col, ctx);
            let total = eq_col + 3 + first_line_len + trailing_comment_len;
            if total <= ctx.options.max_width {
                return Doc::Concat(vec![Doc::text("="), Doc::space(), expr_doc]);
            }
        }

        let is_chain = expr_node.kind() == "navigation_expression"
            || (expr_node.kind() == "call_expression"
                && expr_node
                    .child(0)
                    .is_some_and(|c| c.kind() == "navigation_expression"));
        let is_broken = is_chain
            && crate::lower::chain::is_chain_broken(
                &expr_node,
                ctx,
                collapsed_text_len(ctx.node_text(&expr_node)) + trailing_comment_len,
            );
        if is_broken {
            return Doc::Concat(vec![
                Doc::text("="),
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, expr_doc])),
            ]);
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
