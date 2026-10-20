use tree_sitter::Node;

use crate::doc::Doc;
pub(crate) use crate::lower::call_args::calculate_chain_end;
pub use crate::lower::call_args::{lower_call_suffix, lower_call_suffix_parts};
use crate::lower::context::LowerContext;
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
    is_breakable: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let (call_prefix, val_args, trailing_lambda) = lower_call_suffix_parts(last_suffix, ctx);
    let chain_end = calculate_chain_end(&node, last_suffix, trailing_lambda.is_some(), ctx);
    let chain_len = crate::lower::expression_body::collapsed_text_len(
        ctx.source.get(node.start_byte()..chain_end).unwrap_or(""),
    );

    let is_broken = is_chain_broken(&node, ctx, chain_len) || op_doc.has_hardline();

    let chain_body = Doc::Concat(vec![
        op_doc,
        Doc::continuation_indent(Doc::Concat(suf_docs)),
    ]);
    let chain_group = if is_broken && is_breakable {
        chain_body
    } else {
        Doc::Group(Box::new(chain_body))
    };
    let mut chain_parts = vec![chain_group];
    let prefix_doc = if is_broken && is_breakable {
        Doc::continuation_indent(call_prefix)
    } else {
        call_prefix
    };
    chain_parts.push(prefix_doc);
    if let Some(va) = val_args {
        let va_doc = if is_broken && is_breakable {
            Doc::continuation_indent(va)
        } else {
            va
        };
        chain_parts.push(va_doc);
    }
    let chain_head = Doc::Concat(chain_parts);
    let val_args_breaks = last_suffix
        .children(&mut last_suffix.walk())
        .find(|c| c.kind() == "value_arguments")
        .is_some_and(|va| {
            va.start_position().row != va.end_position().row
                || va.byte_range().len() > ctx.options.max_width
        });
    let already_continuation_indented = val_args_breaks && ctx.options.block_indent != 4;
    match trailing_lambda {
        Some(l) => {
            let lambda_doc = if is_broken && !already_continuation_indented {
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

fn is_type_call_overflow(
    node: &Node<'_>,
    op: &Node<'_>,
    suf: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    if crate::lower::call_args::call_has_args(node) {
        let head_len = ctx.node_text(op).len() + ctx.node_text(suf).len() + 1;
        let start_col = crate::lower::chain::resolve_chain_start_col(node, ctx, head_len);
        start_col + head_len > ctx.options.max_width
    } else {
        let full_len = crate::lower::expression_body::collapsed_text_len(ctx.node_text(node));
        let start_col = crate::lower::chain::resolve_chain_start_col(node, ctx, full_len);
        start_col + full_len > ctx.options.max_width
    }
}

fn is_chained_call(
    node: &Node<'_>,
    op: &Node<'_>,
    suf: &Node<'_>,
    has_comments: bool,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let is_type_name = is_type_name_receiver(op, ctx.source);
    has_comments
        || is_call_receiver(op)
        || (!is_type_name
            && op.kind() == "navigation_expression"
            && (!is_type_name_receiver(suf, ctx.source) || is_part_of_call_chain(node)))
        || (!is_type_name
            && !is_super_or_this(op)
            && ctx.node_text(op).len() >= ctx.options.continuation_indent
            && (is_part_of_call_chain(node) || ctx.has_newline_between(op, suf)))
        || (is_type_name
            && op.kind() == "navigation_expression"
            && !is_type_name_receiver(suf, ctx.source)
            && !is_part_of_call_chain(node)
            && is_type_call_overflow(node, op, suf, ctx))
}

fn collect_nav_comment_docs<'a>(
    op: Node<'a>,
    nav_comments: Vec<Node<'a>>,
    suf_doc: Doc<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Vec<Doc<'a>> {
    let mut suf_docs = Vec::new();
    let mut prev = op;
    for c in nav_comments {
        if ctx.has_newline_between(&prev, &c) {
            suf_docs.push(Doc::HardLine);
        } else {
            suf_docs.push(Doc::space());
        }
        suf_docs.push(lower_node(c, ctx));
        prev = c;
    }
    suf_docs.push(suf_doc);
    suf_docs
}

fn lower_chained_call_expression<'a>(
    node: Node<'a>,
    callee_node: Node<'a>,
    suffixes: &[Node<'a>],
    ctx: &LowerContext<'a, '_>,
) -> Option<Doc<'a>> {
    let (Some(raw_op), nav_comments, Some(suf)) = parse_callee_navigation(callee_node) else {
        return None;
    };
    let op = ctx.resolve_leaf(raw_op);

    if !is_chained_call(&node, &op, &suf, !nav_comments.is_empty(), ctx) {
        return None;
    }

    let op_doc = lower_node(op, ctx);
    let has_multiline_args = crate::lower::delegates::call_has_multiline_args(&node, ctx);
    let is_opening_call_on_simple_receiver = op.kind() == "simple_identifier";
    let is_breakable = (!nav_comments.is_empty() || !is_super_or_this(&op))
        && (!is_opening_call_on_simple_receiver
            || !has_multiline_args
            || ctx.has_newline_between(&op, &suf));
    // In Google/ktfmt style (block_indent != 4), calls following a multiline raw string
    // (such as .trimIndent() or .trimMargin()) must wrap to a new line.
    // In Kotlinlang/ktlint style (block_indent == 4), they attach directly to the closing quotes.
    let is_raw_string = ctx.options.block_indent != 4
        && (matches!(op.kind(), "multiline_string_literal")
            || (op.kind() == "string_literal" && op.start_position().row != op.end_position().row));
    let suf_doc = lower_navigation_suffix_with_break(suf, ctx, is_breakable, is_raw_string);
    let mut suf_docs = collect_nav_comment_docs(op, nav_comments, suf_doc, ctx);

    let is_root = is_navigation_chain_root(&node);
    if is_root
        && let Some(last) = suffixes.last()
        && last.kind() == "call_suffix"
    {
        for s in &suffixes[..suffixes.len() - 1] {
            suf_docs.push(lower_node(*s, ctx));
        }
        return Some(lower_chain_root_with_call_suffix(
            node,
            *last,
            op_doc,
            suf_docs,
            is_breakable,
            ctx,
        ));
    }

    for s in suffixes {
        suf_docs.push(lower_node(*s, ctx));
    }
    let chain_doc = if is_breakable {
        Doc::Concat(vec![
            op_doc,
            Doc::continuation_indent(Doc::Concat(suf_docs)),
        ])
    } else {
        Doc::Concat(vec![op_doc, Doc::Concat(suf_docs)])
    };
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
