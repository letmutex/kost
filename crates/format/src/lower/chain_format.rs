use tree_sitter::Node;

use super::chain_call::LastCallData;
use super::chain_step::{ChainStep, PostOp};
use crate::doc::Doc;
use crate::lower::context::LowerContext;

pub(crate) fn lower_step_selector<'a>(
    step: &ChainStep<'a>,
    in_prefix: bool,
    has_comments: bool,
    has_intermediate_lambda: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut selector_docs = Vec::new();
    for (c_idx, c) in step.comments.iter().enumerate() {
        if c_idx > 0
            && crate::rules::comments::try_merge_continuation_comment(
                Some(step.comments[c_idx - 1]),
                c,
                &mut selector_docs,
                ctx,
            )
        {
            continue;
        }
        if c_idx == 0
            && step.prev_end_byte > 0
            && !ctx.source[step.prev_end_byte..c.start_byte()].contains('\n')
            && matches!(c.kind(), "comment" | "line_comment")
        {
            selector_docs.push(Doc::space());
            selector_docs.push(Doc::line_comment(
                ctx.node_text(c),
                true,
                ctx.style.align_wrapped_trailing_comments,
            ));
        } else {
            selector_docs.push(Doc::HardLine);
            selector_docs.push(lower_fn(*c, ctx));
        }
    }

    if let Some((op, ta, target)) = step.lead_op {
        if let Some(ta_node) = ta {
            selector_docs.push(lower_fn(ta_node, ctx));
        }
        if op == "::" {
            selector_docs.push(Doc::text("::"));
            selector_docs.push(Doc::concat(vec![Doc::SoftLine, lower_fn(target, ctx)]));
        } else {
            if !in_prefix {
                if has_comments || has_intermediate_lambda {
                    selector_docs.push(Doc::HardLine);
                } else {
                    selector_docs.push(Doc::SoftLine);
                }
            }
            selector_docs.push(Doc::text(op));
            selector_docs.push(lower_fn(target, ctx));
        }
    }
    selector_docs
}

pub(crate) fn is_lambda_multiline(node: Node<'_>) -> bool {
    if node.start_position().row == node.end_position().row {
        return false;
    }
    let target = if node.kind() == "annotated_lambda" {
        node.children(&mut node.walk())
            .find(|c| c.kind() == "lambda_literal")
            .unwrap_or(node)
    } else {
        node
    };
    target.children(&mut target.walk()).any(|c| {
        !matches!(c.kind(), "{" | "}" | "->" | "lambda_parameters")
            && (c.kind() != "statements" || c.child_count() > 0)
    })
}

fn should_keep_in_chain_group<'a>(
    step: &ChainStep<'a>,
    steps: &[ChainStep<'a>],
    in_prefix: bool,
) -> bool {
    if in_prefix {
        return false;
    }
    let total_steps = steps.len();
    let lambda_count = steps
        .iter()
        .filter(|s| s.call.as_ref().is_some_and(|c| c.2.is_some()))
        .count();
    let last_lambda_is_single_line = step
        .call
        .and_then(|c| c.2)
        .is_some_and(|l| !is_lambda_multiline(l));
    lambda_count > 1
        && last_lambda_is_single_line
        && !steps[..total_steps - 1].iter().any(|s| {
            s.call
                .as_ref()
                .is_some_and(|c| c.2.is_some_and(is_lambda_multiline))
        })
}

fn lower_last_call_args<'a>(
    ta: Option<Node<'a>>,
    va: Option<Node<'a>>,
    tl: Option<Node<'a>>,
    last_call: &mut Option<LastCallData<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut call_docs = Vec::new();
    let ta_gid = ctx.next_group_id();
    if let Some(ta_node) = ta {
        call_docs.push(Doc::group_with_id(ta_gid, lower_fn(ta_node, ctx)));
    }
    if let Some(va_node) = va {
        let v_gid = ctx.next_group_id();
        let mut va_doc = lower_fn(va_node, ctx);
        if ta.is_some() {
            va_doc = Doc::if_group_break(
                ta_gid,
                super::params::lower_value_arguments_multiline(va_node, ctx, lower_fn),
                va_doc,
            );
        }
        call_docs.push(Doc::group_with_id(v_gid, va_doc));
    }
    if let Some(tl_node) = tl {
        *last_call = Some((None, None, Some(tl_node)));
    }
    call_docs
}

pub(crate) fn lower_step_call<'a>(
    step: &ChainStep<'a>,
    idx: usize,
    steps: &[ChainStep<'a>],
    receiver: &Node<'a>,
    in_prefix: bool,
    break_scoping_root: bool,
    is_on_rhs: &mut Option<bool>,
    last_call: &mut Option<LastCallData<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut call_docs = Vec::new();
    let total_steps = steps.len();
    let total_calls = steps.iter().filter(|s| s.call.is_some()).count();
    let has_trailing_lambda = steps
        .last()
        .is_some_and(|s| s.call.as_ref().is_some_and(|c| c.2.is_some()));
    let is_last = idx + 1 == total_steps;

    if is_last
        && step.post_ops.is_empty()
        && let Some((ta, va, tl)) = step.call
        && !in_prefix
        && total_calls > 1
        && !has_trailing_lambda
    {
        call_docs = lower_last_call_args(ta, va, tl, last_call, ctx, lower_fn);
    } else if is_last
        && step.post_ops.is_empty()
        && step.call.is_some()
        && !should_keep_in_chain_group(step, steps, in_prefix)
    {
        *last_call = step.call;
    } else if let Some((ta, va, tl)) = step.call {
        if let Some(ta_node) = ta {
            call_docs.push(lower_fn(ta_node, ctx));
        }
        if let Some(va_node) = va {
            let va_doc = lower_fn(va_node, ctx);
            let next_is_chained = steps
                .get(idx + 1)
                .is_some_and(|next| next.lead_op.is_some());
            if !ctx.options.use_experimental_engine && in_prefix && next_is_chained {
                call_docs.push(Doc::continuation_indent(va_doc));
            } else {
                call_docs.push(va_doc);
            }
        }
        if let Some(tl_node) = tl {
            call_docs.push(Doc::space());
            if idx == 0 && break_scoping_root {
                let mut tl_doc = super::lambda::lower_lambda_node_multiline(tl_node, ctx, lower_fn);
                if ctx.options.continuation_indent == 2 && total_steps > 1 {
                    let on_rhs = *is_on_rhs
                        .get_or_insert_with(|| super::chain_call::is_receiver_on_rhs(*receiver));
                    if !on_rhs {
                        tl_doc = Doc::continuation_indent(tl_doc);
                    }
                }
                call_docs.push(tl_doc);
            } else {
                call_docs.push(lower_fn(tl_node, ctx));
            }
        }
    }
    call_docs
}

pub(crate) fn lower_step_post_ops<'a>(
    step: &ChainStep<'a>,
    in_prefix: bool,
    is_last: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut call_docs = Vec::new();
    let mut last_postfix: Option<&str> = None;
    for post in &step.post_ops {
        match post {
            PostOp::Indexing(node) => {
                last_postfix = None;
                let idx_doc = lower_fn(*node, ctx);
                if in_prefix && !is_last {
                    call_docs.push(Doc::continuation_indent(idx_doc));
                } else {
                    call_docs.push(idx_doc);
                }
            }
            PostOp::Postfix(op) => {
                if let Some(prev) = last_postfix
                    && prev.chars().last() == op.chars().next()
                {
                    call_docs.push(Doc::space());
                }
                last_postfix = Some(op);
                call_docs.push(Doc::text(op));
            }
        }
    }
    call_docs
}

pub(crate) fn format_string_receiver_single_step<'a>(
    receiver_doc: Doc<'a>,
    step: &ChainStep<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Option<Doc<'a>> {
    let (op, type_args, target) = step.lead_op?;
    let mut sel_parts = Vec::new();
    if let Some(ta_node) = type_args {
        sel_parts.push(lower_fn(ta_node, ctx));
    }
    sel_parts.push(lower_fn(target, ctx));
    if let Some((ta, va, tl)) = step.call {
        if let Some(ta_node) = ta {
            sel_parts.push(lower_fn(ta_node, ctx));
        }
        if let Some(va_node) = va {
            sel_parts.push(lower_fn(va_node, ctx));
        }
        if let Some(tl_node) = tl {
            sel_parts.push(Doc::space());
            sel_parts.push(lower_fn(tl_node, ctx));
        }
    }
    sel_parts.extend(lower_step_post_ops(step, false, true, ctx, lower_fn));
    let mut lead_docs = Vec::new();
    if step.comments.is_empty() {
        lead_docs.push(Doc::SoftLine);
    } else {
        for c in &step.comments {
            lead_docs.push(Doc::HardLine);
            lead_docs.push(lower_fn(*c, ctx));
        }
        lead_docs.push(Doc::HardLine);
    }
    lead_docs.push(Doc::text(op));
    lead_docs.push(Doc::concat(sel_parts));

    Some(Doc::group(Doc::concat(vec![
        receiver_doc,
        Doc::continuation_indent(Doc::concat(lead_docs)),
    ])))
}
