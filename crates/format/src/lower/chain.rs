use tree_sitter::Node;

use super::chain_call::{
    ChainAnalysis, LastCallData, accumulate_chain_step, analyze_chain, format_elvis_receiver,
    format_last_call, is_receiver_on_rhs, should_prefix_continue,
};
use super::chain_format::{
    format_string_receiver_single_step, lower_step_call, lower_step_post_ops, lower_step_selector,
};
use super::chain_step::{ChainStep, assemble_chain_steps};
pub use super::chain_step::{SpineSegment, flatten_spine, is_spine_root};
use crate::doc::Doc;
use crate::lower::context::LowerContext;

struct ChainStepState<'a> {
    in_prefix: bool,
    prefix_doc: Doc<'a>,
    tail_docs: Vec<Doc<'a>>,
    last_call: Option<LastCallData<'a>>,
    last_call_prefix_gid: Option<crate::doc::GroupId>,
    prev_is_call: bool,
    prev_name: Option<&'a str>,
    prev_is_super_or_this: bool,
    prev_op: Option<&'a str>,
}

fn lower_step_call_and_post_ops<'a>(
    step: &ChainStep<'a>,
    idx: usize,
    steps: &[ChainStep<'a>],
    receiver: &Node<'a>,
    state: &mut ChainStepState<'a>,
    analysis: &ChainAnalysis,
    is_on_rhs: &mut Option<bool>,
    is_last: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Vec<Doc<'a>> {
    let mut call_docs = lower_step_call(
        step,
        idx,
        steps,
        receiver,
        state.in_prefix,
        analysis.break_scoping_root,
        is_on_rhs,
        &mut state.last_call,
        ctx,
        lower_fn,
    );
    call_docs.extend(lower_step_post_ops(
        step,
        state.in_prefix,
        is_last,
        ctx,
        lower_fn,
    ));
    call_docs
}

fn format_single_chain_step<'a>(
    step: &ChainStep<'a>,
    idx: usize,
    steps: &[ChainStep<'a>],
    analysis: &ChainAnalysis,
    receiver: &Node<'a>,
    is_on_rhs: &mut Option<bool>,
    state: &mut ChainStepState<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let total_steps = steps.len();
    let is_last = idx + 1 == total_steps;
    let curr_target_name = step.lead_op.as_ref().map(|(_, _, t)| ctx.node_text(t));
    let curr_op = step.lead_op.as_ref().map(|(o, _, _)| *o);
    let curr_has_call = step.call.is_some();

    if state.in_prefix
        && !should_prefix_continue(
            step,
            idx,
            steps,
            receiver,
            state.prev_name,
            state.prev_is_call,
            state.prev_is_super_or_this,
            state.prev_op,
            analysis.has_trailing_lambda,
            analysis.has_intermediate_break,
            ctx,
        )
    {
        state.in_prefix = false;
    }

    let selector_docs = lower_step_selector(
        step,
        state.in_prefix,
        analysis.has_comments,
        analysis.has_intermediate_lambda,
        ctx,
        lower_fn,
    );
    let call_docs = lower_step_call_and_post_ops(
        step, idx, steps, receiver, state, analysis, is_on_rhs, is_last, ctx, lower_fn,
    );

    accumulate_chain_step(
        &mut state.prefix_doc,
        &mut state.tail_docs,
        &mut state.last_call_prefix_gid,
        selector_docs,
        call_docs,
        state.in_prefix,
        is_last,
        curr_has_call,
        idx,
        total_steps,
        step.lead_op.is_none(),
        analysis.has_trailing_lambda,
        analysis.has_intermediate_break,
        crate::engine::node_kind(*receiver),
        step.call.is_none(),
        ctx,
    );

    state.prev_is_call = curr_has_call;
    state.prev_name = curr_target_name;
    state.prev_is_super_or_this = false;
    state.prev_op = curr_op;
}

pub(crate) fn format_chain_body<'a>(
    receiver: Node<'a>,
    receiver_doc: Doc<'a>,
    steps: &[ChainStep<'a>],
    mut is_on_rhs: Option<bool>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> (
    Doc<'a>,
    Vec<Doc<'a>>,
    Option<LastCallData<'a>>,
    Option<crate::doc::GroupId>,
) {
    let analysis = analyze_chain(steps, &receiver, ctx);
    let recv_kind = crate::engine::node_kind(receiver);
    let mut state = ChainStepState {
        in_prefix: true,
        prefix_doc: receiver_doc,
        tail_docs: Vec::new(),
        last_call: None,
        last_call_prefix_gid: None,
        prev_is_call: false,
        prev_name: if recv_kind == "simple_identifier" {
            Some(ctx.node_text(&receiver))
        } else {
            None
        },
        prev_is_super_or_this: matches!(recv_kind, "this_expression" | "super_expression"),
        prev_op: None,
    };

    for (idx, step) in steps.iter().enumerate() {
        format_single_chain_step(
            step,
            idx,
            steps,
            &analysis,
            &receiver,
            &mut is_on_rhs,
            &mut state,
            ctx,
            lower_fn,
        );
    }

    (
        state.prefix_doc,
        state.tail_docs,
        state.last_call,
        state.last_call_prefix_gid,
    )
}

pub fn format_chain_spine<'a>(
    receiver: Node<'a>,
    segments: Vec<SpineSegment<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let receiver_kind = crate::engine::node_kind(receiver);
    if matches!(receiver_kind, "infix_expression" | "elvis_expression")
        && !segments.is_empty()
        && let Some(doc) = format_elvis_receiver(receiver, segments.clone(), ctx, lower_fn)
    {
        return doc;
    }
    let mut receiver_doc = lower_fn(receiver, ctx);
    if segments.is_empty() {
        return receiver_doc;
    }
    let mut is_on_rhs = None;
    if matches!(
        receiver_kind,
        "if_expression" | "when_expression" | "try_expression"
    ) {
        let rhs = is_receiver_on_rhs(receiver);
        is_on_rhs = Some(rhs);
        if rhs {
            receiver_doc = Doc::continuation_indent(receiver_doc);
        }
    }

    let steps = assemble_chain_steps(segments);
    if matches!(receiver_kind, "string_literal" | "multiline_string_literal")
        && steps.len() == 1
        && let Some(doc) =
            format_string_receiver_single_step(receiver_doc.clone(), &steps[0], ctx, lower_fn)
    {
        return doc;
    }
    let (prefix_doc, tail_docs, last_call, last_call_prefix_gid) =
        format_chain_body(receiver, receiver_doc, &steps, is_on_rhs, ctx, lower_fn);

    let gid = ctx.next_group_id();
    let has_tail = !tail_docs.is_empty();
    let chain_group = if !has_tail {
        Doc::group_with_id(gid, prefix_doc)
    } else {
        Doc::group_with_id(
            gid,
            Doc::concat(vec![
                prefix_doc,
                Doc::continuation_indent(Doc::concat(tail_docs)),
            ]),
        )
    };

    if let Some(last_call_data) = last_call {
        format_last_call(
            chain_group,
            last_call_data,
            has_tail,
            gid,
            last_call_prefix_gid,
            ctx,
            lower_fn,
        )
    } else {
        chain_group
    }
}
