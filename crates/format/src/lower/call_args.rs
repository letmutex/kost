use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lists::has_trailing_comma;
use crate::lower::lower_node;

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

pub(crate) fn compute_call_continuation_indent(
    node: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    let mut start_col = ctx.compute_target_indent(node);
    let mut curr_node = *node;
    while let Some(parent) = curr_node.parent() {
        if matches!(
            parent.kind(),
            "assignment" | "property_declaration" | "property_delegate" | "function_body"
        ) {
            start_col += ctx.options.continuation_indent;
            break;
        }
        if matches!(parent.kind(), "statements" | "class_body") {
            break;
        }
        curr_node = parent;
    }
    start_col
}

pub(crate) fn call_has_multiline_args(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut suffix_cursor = child.walk();
            for suffix_child in child.children(&mut suffix_cursor) {
                if suffix_child.kind() == "value_arguments" {
                    let has_arg = suffix_child
                        .children(&mut suffix_child.walk())
                        .any(|c| c.kind() == "value_argument");
                    if !has_arg {
                        return false;
                    }
                    if suffix_child.start_position().row != suffix_child.end_position().row
                        || crate::lower::lists::has_trailing_comma(&suffix_child)
                    {
                        return true;
                    }
                    let start_col = compute_call_continuation_indent(node, ctx);
                    let text = ctx
                        .source
                        .get(node.start_byte()..suffix_child.end_byte())
                        .unwrap_or("");
                    let call_header_len =
                        start_col + crate::lower::expression_body::collapsed_text_len(text);
                    return call_header_len > ctx.options.max_width;
                }
            }
        }
    }
    false
}

pub(crate) fn find_innermost_call<'a>(node: Node<'a>) -> Option<Node<'a>> {
    let mut curr = node;
    let mut root_call = None;
    while matches!(
        curr.kind(),
        "navigation_expression" | "call_expression" | "postfix_expression" | "indexing_expression"
    ) {
        if curr.kind() == "call_expression" {
            root_call = Some(curr);
        }
        if let Some(first) = curr.child(0)
            && matches!(
                first.kind(),
                "navigation_expression"
                    | "call_expression"
                    | "postfix_expression"
                    | "indexing_expression"
            )
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
                        return ctx
                            .source
                            .get(base.start_byte()..=sub.start_byte())
                            .map(crate::lower::expression_body::collapsed_text_len);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn calculate_chain_end(
    node: &Node<'_>,
    last_suffix: Node<'_>,
    has_trailing_lambda: bool,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    let mut cursor = last_suffix.walk();
    let mut type_args = None;
    let mut val_args = None;
    let mut lambda = None;
    for child in last_suffix.children(&mut cursor) {
        if child.kind() == "type_arguments" {
            type_args = Some(child);
        } else if child.kind() == "value_arguments" {
            val_args = Some(child);
        } else if matches!(child.kind(), "annotated_lambda" | "lambda_literal") {
            lambda = Some(child);
        }
    }

    if let Some(ta) = type_args
        && (ta.start_position().row != ta.end_position().row
            || crate::lower::chain::call_has_multiline_type_args(node, ctx))
    {
        return ta.start_byte() + 1;
    }

    if let Some(va) = val_args {
        let va_breaks = crate::lower::lists::has_trailing_comma(&va)
            || (va.start_position().row != va.end_position().row
                && va
                    .children(&mut va.walk())
                    .any(|c| c.kind() == "value_argument"))
            || call_has_multiline_args(node, ctx);
        if va_breaks {
            return va.start_byte();
        }
    }

    if has_trailing_lambda && let Some(c) = lambda {
        let l = if c.kind() == "annotated_lambda" {
            c.children(&mut c.walk())
                .find(|sub| sub.kind() == "lambda_literal")
                .unwrap_or(c)
        } else {
            c
        };
        return find_lambda_first_line_end_byte(l);
    }

    last_suffix.end_byte()
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

pub(crate) fn format_inter_comments<'a>(
    inter_comments: &[Node<'a>],
    val_args_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> (Vec<Doc<'a>>, Option<Node<'a>>) {
    let mut parts = Vec::new();
    let mut prev = val_args_node;
    for c in inter_comments {
        if prev.is_some_and(|p| ctx.has_newline_between(&p, c)) {
            parts.push(Doc::indent(Doc::HardLine));
        } else {
            parts.push(Doc::space());
        }
        parts.push(lower_node(*c, ctx));
        prev = Some(*c);
    }
    (parts, prev)
}

pub(crate) fn format_call_suffix_lambda<'a>(
    lambda_node: Option<Node<'a>>,
    inter_comments: Vec<Node<'a>>,
    val_args_node: Option<Node<'a>>,
    val_args_breaks: bool,
    ctx: &LowerContext<'a, '_>,
) -> Option<Doc<'a>> {
    let Some(l_node) = lambda_node else {
        if inter_comments.is_empty() {
            return None;
        }
        let (parts, _) = format_inter_comments(&inter_comments, val_args_node, ctx);
        return Some(Doc::Concat(parts));
    };

    let raw_lambda = lower_node(l_node, ctx);
    let l = if val_args_node.is_some() && val_args_breaks && ctx.options.block_indent != 4 {
        Doc::continuation_indent(raw_lambda)
    } else {
        raw_lambda
    };
    if inter_comments.is_empty() {
        return Some(Doc::Concat(vec![Doc::space(), l]));
    }
    let (mut parts, prev) = format_inter_comments(&inter_comments, val_args_node, ctx);
    if prev.is_some_and(|p| ctx.has_newline_between(&p, &l_node)) {
        parts.push(Doc::HardLine);
    } else {
        parts.push(Doc::space());
    }
    parts.push(l);
    Some(Doc::Concat(parts))
}

pub fn lower_call_suffix_parts<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> (Doc<'a>, Option<Doc<'a>>, Option<Doc<'a>>) {
    let mut val_args_node = None;
    let mut val_args_breaks = false;
    let mut val_args_doc = None;
    let mut lambda_node = None;
    let mut inter_comments = Vec::new();
    let mut other_docs = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "value_arguments" {
            val_args_node = Some(child);
            let trailing = has_trailing_comma(&child);
            val_args_breaks = !trailing
                && (child.start_position().row != child.end_position().row
                    || child.byte_range().len() > ctx.options.max_width);
            val_args_doc = Some(lower_node(child, ctx));
        } else if kind == "annotated_lambda" || kind == "lambda_literal" {
            lambda_node = Some(child);
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment")
            && val_args_node.is_some()
            && lambda_node.is_none()
        {
            inter_comments.push(child);
        } else {
            other_docs.push(lower_node(child, ctx));
        }
    }

    let lambda = format_call_suffix_lambda(
        lambda_node,
        inter_comments,
        val_args_node,
        val_args_breaks,
        ctx,
    );
    (Doc::Concat(other_docs), val_args_doc, lambda)
}

pub fn lower_call_suffix<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let (prefix, val_args, lambda) = lower_call_suffix_parts(node, ctx);
    let mut docs = vec![prefix];
    if let Some(va) = val_args {
        docs.push(va);
    }
    if let Some(l) = lambda {
        docs.push(l);
    }
    Doc::Concat(docs)
}
