use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lists::has_trailing_comma;
use crate::lower::lower_node;
use crate::lower::navigation::{
    count_chain_lambdas, is_call_receiver, is_chain_broken, is_navigation_chain_root,
    is_part_of_call_chain, is_super_or_this, is_type_name_receiver,
    lower_navigation_suffix_with_break,
};

pub fn lower_call_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut callee = None;
    let mut suffixes = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if callee.is_none() {
            callee = Some(child);
        } else {
            suffixes.push(child);
        }
    }

    let Some(callee_node) = callee else {
        return Doc::empty();
    };

    if callee_node.kind() == "navigation_expression"
        && let Some(doc) = lower_chained_call_expression(node, callee_node, &suffixes, ctx)
    {
        return doc;
    }

    let mut docs = Vec::new();
    docs.push(lower_node(callee_node, ctx));
    for s in suffixes {
        docs.push(lower_node(s, ctx));
    }
    Doc::Concat(docs)
}

fn lower_chain_root_with_call_suffix<'a>(
    node: Node<'a>,
    last_suffix: Node<'a>,
    op_doc: Doc<'a>,
    suf_docs: Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let (call_prefix, val_args, trailing_lambda) = lower_call_suffix_parts(last_suffix, ctx);
    let chain_end = last_suffix
        .children(&mut last_suffix.walk())
        .find(|c| {
            matches!(
                c.kind(),
                "type_arguments" | "value_arguments" | "annotated_lambda" | "lambda_literal"
            )
        })
        .map(|c| c.start_byte())
        .unwrap_or(last_suffix.end_byte());
    let chain_len = chain_end.saturating_sub(node.start_byte());
    let is_broken = is_chain_broken(&node, ctx, chain_len) || op_doc.has_hardline();

    let chain_body = Doc::Concat(vec![
        op_doc,
        Doc::continuation_indent(Doc::Concat(suf_docs)),
    ]);
    let chain_group = if is_broken {
        chain_body
    } else {
        Doc::Group(Box::new(chain_body))
    };
    let mut chain_parts = vec![chain_group];
    let prefix_doc = if is_broken {
        Doc::continuation_indent(call_prefix)
    } else {
        call_prefix
    };
    chain_parts.push(prefix_doc);
    if let Some(va) = val_args {
        let va_doc = if is_broken {
            Doc::continuation_indent(va)
        } else {
            va
        };
        chain_parts.push(va_doc);
    }
    let chain_head = Doc::Concat(chain_parts);
    match trailing_lambda {
        Some(l) => {
            let lambda_doc = if is_broken {
                Doc::continuation_indent(l)
            } else {
                l
            };
            Doc::Concat(vec![chain_head, lambda_doc])
        }
        None => chain_head,
    }
}

fn parse_callee_navigation<'a>(
    callee_node: Node<'a>,
) -> (Option<Node<'a>>, Vec<Node<'a>>, Option<Node<'a>>) {
    let mut nav_op = None;
    let mut nav_comments = Vec::new();
    let mut nav_suf = None;
    let mut nav_cursor = callee_node.walk();
    for child in callee_node.children(&mut nav_cursor) {
        if nav_op.is_none() {
            nav_op = Some(child);
        } else if matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            nav_comments.push(child);
        } else {
            nav_suf = Some(child);
        }
    }
    (nav_op, nav_comments, nav_suf)
}

fn lower_chained_call_expression<'a>(
    node: Node<'a>,
    callee_node: Node<'a>,
    suffixes: &[Node<'a>],
    ctx: &LowerContext<'a, '_>,
) -> Option<Doc<'a>> {
    let (Some(op), nav_comments, Some(suf)) = parse_callee_navigation(callee_node) else {
        return None;
    };

    let is_chain = !nav_comments.is_empty()
        || is_call_receiver(&op)
        || op.kind() == "navigation_expression"
        || (!is_type_name_receiver(&op, ctx.source)
            && !is_super_or_this(&op)
            && ctx.node_text(&op).len() >= ctx.options.continuation_indent
            && (is_part_of_call_chain(&node) || ctx.has_newline_between(&op, &suf)));
    if !is_chain {
        return None;
    }

    let op_doc = lower_node(op, ctx);
    let is_breakable = !nav_comments.is_empty() || !is_super_or_this(&op);
    let is_raw_string = matches!(op.kind(), "multiline_string_literal")
        || (op.kind() == "string_literal" && op.start_position().row != op.end_position().row);
    let suf_doc = lower_navigation_suffix_with_break(suf, ctx, is_breakable, is_raw_string);
    let mut suf_docs = Vec::new();
    for c in nav_comments {
        suf_docs.push(Doc::HardLine);
        suf_docs.push(lower_node(c, ctx));
    }
    suf_docs.push(suf_doc);

    let is_root = is_navigation_chain_root(&node);
    if is_root
        && let Some(last) = suffixes.last()
        && last.kind() == "call_suffix"
    {
        for s in &suffixes[..suffixes.len() - 1] {
            suf_docs.push(lower_node(*s, ctx));
        }
        return Some(lower_chain_root_with_call_suffix(
            node, *last, op_doc, suf_docs, ctx,
        ));
    }

    for s in suffixes {
        suf_docs.push(lower_node(*s, ctx));
    }
    let chain_doc = Doc::Concat(vec![
        op_doc,
        Doc::continuation_indent(Doc::Concat(suf_docs)),
    ]);
    let result = if is_root {
        if count_chain_lambdas(&node) > 1 {
            chain_doc
        } else {
            Doc::Group(Box::new(chain_doc))
        }
    } else {
        chain_doc
    };
    Some(result)
}

fn format_call_suffix_lambda<'a>(
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
        let mut parts = Vec::new();
        let mut prev = val_args_node;
        for c in &inter_comments {
            if prev.is_some_and(|p| ctx.has_newline_between(&p, c)) {
                parts.push(Doc::indent(Doc::HardLine));
            } else {
                parts.push(Doc::space());
            }
            parts.push(lower_node(*c, ctx));
            prev = Some(*c);
        }
        return Some(Doc::Concat(parts));
    };

    let raw_lambda = lower_node(l_node, ctx);
    let l = if val_args_node.is_some() && val_args_breaks {
        Doc::continuation_indent(raw_lambda)
    } else {
        raw_lambda
    };
    if inter_comments.is_empty() {
        return Some(Doc::Concat(vec![Doc::space(), l]));
    }
    let mut parts = Vec::new();
    let mut prev = val_args_node;
    for c in &inter_comments {
        if prev.is_some_and(|p| ctx.has_newline_between(&p, c)) {
            parts.push(Doc::indent(Doc::HardLine));
        } else {
            parts.push(Doc::space());
        }
        parts.push(lower_node(*c, ctx));
        prev = Some(*c);
    }
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
            let text = ctx.node_text(&child);
            val_args_breaks =
                !trailing && (text.contains('\n') || text.len() > ctx.options.max_width);
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
