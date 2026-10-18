use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::assignment::has_trailing_lambda_child;
use crate::lower::context::LowerContext;
use crate::lower::delegates::call_has_multiline_args;
use crate::lower::lower_node;

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

fn chain_root_element<'a>(node: Node<'a>) -> Node<'a> {
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
        let Some(receiver) = nav.child(0) else {
            break;
        };
        if matches!(receiver.kind(), "call_expression" | "navigation_expression") {
            curr = receiver;
            continue;
        }
        break;
    }
    curr
}

fn find_lambda_first_line_end_byte(lambda: Node<'_>) -> usize {
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

fn expression_first_line_len(node: &Node<'_>) -> usize {
    let root = chain_root_element(*node);
    if let Some(call) = crate::lower::delegates::find_innermost_call(root)
        && call_has_multiline_args(&call)
        && let Some(len) = crate::lower::delegates::find_call_opening_paren_len(call, root)
    {
        return len;
    }
    if let Some(lambda) = crate::lower::delegates::find_first_lambda(root) {
        return find_lambda_first_line_end_byte(lambda).saturating_sub(root.start_byte());
    }
    root.byte_range().len()
}

pub fn format_expression_body<'a>(
    node: Node<'a>,
    expr_child: Option<Node<'a>>,
    comments_before_expr: Vec<Node<'a>>,
    comments_after_expr: Vec<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    header_col: Option<usize>,
) -> Doc<'a> {
    let is_pascal_call = expr_child.is_some_and(|c| is_pascal_case_call(&c, ctx.source));
    let mut expr_doc = expr_child
        .map(|c| lower_node(c, ctx))
        .unwrap_or_else(Doc::empty);
    if !comments_after_expr.is_empty() {
        let mut after_parts = vec![expr_doc];
        let mut prev = expr_child;
        for c in comments_after_expr {
            if prev.is_some_and(|p| p.end_position().row == c.start_position().row) {
                after_parts.push(Doc::space());
            } else {
                after_parts.push(Doc::HardLine);
            }
            after_parts.push(lower_node(c, ctx));
            prev = Some(c);
        }
        expr_doc = Doc::Concat(after_parts);
    }
    if !comments_before_expr.is_empty() {
        let mut inner = Vec::new();
        for c in comments_before_expr {
            inner.push(Doc::HardLine);
            inner.push(lower_node(c, ctx));
        }
        if !inner.last().is_some_and(|d| d.ends_with_newline()) {
            inner.push(Doc::HardLine);
        }
        inner.push(expr_doc);
        return Doc::Concat(vec![
            Doc::text("="),
            Doc::continuation_indent(Doc::Concat(inner)),
        ]);
    }
    if is_pascal_call {
        return Doc::Concat(vec![
            Doc::text("="),
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, expr_doc])),
        ]);
    }
    if let Some(e_node) = expr_child {
        if matches!(e_node.kind(), "lambda_literal" | "annotated_lambda") {
            return Doc::Concat(vec![Doc::text("="), Doc::space(), expr_doc]);
        }
        let is_chain_or_multiline_call = e_node.kind() == "navigation_expression"
            || e_node
                .child(0)
                .is_some_and(|c| c.kind() == "navigation_expression")
            || crate::lower::delegates::find_innermost_call(e_node)
                .is_some_and(|c| call_has_multiline_args(&c));
        let is_block_like = (e_node.kind() == "call_expression"
            || e_node.kind() == "navigation_expression")
            && (has_trailing_lambda_child(&e_node)
                || (ctx.options.block_indent == 4 && is_chain_or_multiline_call));
        if is_block_like && !has_complex_or_control_receiver(&e_node) {
            let first_line_len = expression_first_line_len(&e_node);
            let eq_col =
                header_col.unwrap_or_else(|| node.start_position().column.saturating_sub(1));
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
