use tree_sitter::Node;

pub use super::chain_step::SpineSegment;
use crate::doc::Doc;
use crate::lower::context::LowerContext;

pub type LastCallData<'a> = (Option<Node<'a>>, Option<Node<'a>>, Option<Node<'a>>);

pub fn is_receiver_on_rhs(receiver: Node<'_>) -> bool {
    let mut curr = receiver;
    while let Some(p) = curr.parent() {
        if matches!(p.kind(), "block" | "statements" | "source_file") {
            break;
        }
        if matches!(p.kind(), "property_declaration" | "assignment") {
            return true;
        }
        if p.kind() == "function_body" && p.children(&mut p.walk()).any(|c| c.kind() == "=") {
            return true;
        }
        curr = p;
    }
    matches!(
        curr.kind(),
        "property_declaration" | "assignment" | "function_body"
    )
}

pub fn format_elvis_receiver<'a>(
    receiver: Node<'a>,
    segments: Vec<SpineSegment<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Option<Doc<'a>> {
    if receiver.kind() == "infix_expression" && !segments.is_empty() {
        let l = receiver.child(0)?;
        let op = receiver.child(1)?;
        let r = receiver.child(2)?;
        let left_doc = lower_fn(l, ctx);
        let op_text = ctx.node_text(&op);
        let (inner_recv, mut inner_segs) = super::chain_step::flatten_spine(r, ctx);
        inner_segs.extend(segments);
        let right_doc = super::chain::format_chain_spine(inner_recv, inner_segs, ctx, lower_fn);
        let gid = ctx.next_group_id();
        return Some(Doc::group_with_id(
            gid,
            Doc::concat(vec![
                left_doc,
                Doc::space(),
                Doc::text(op_text),
                Doc::indent_if_group_breaks(
                    gid,
                    crate::doc::IndentKind::Continuation,
                    Doc::concat(vec![Doc::Line, Doc::fits_first_line(right_doc)]),
                ),
            ]),
        ));
    }
    if receiver.kind() != "elvis_expression" || segments.is_empty() {
        return None;
    }
    let (l, r) = (receiver.child(0)?, receiver.child(2)?);
    let left_doc = lower_fn(l, ctx);
    let (inner_recv, mut inner_segs) = super::chain_step::flatten_spine(r, ctx);
    inner_segs.extend(segments);
    let right_doc = super::chain::format_chain_spine(inner_recv, inner_segs, ctx, lower_fn);
    let gid = ctx.next_group_id();
    Some(Doc::group_with_id(
        gid,
        Doc::concat(vec![
            left_doc,
            Doc::indent_if_group_breaks(
                gid,
                crate::doc::IndentKind::Continuation,
                Doc::concat(vec![Doc::Line, Doc::text("?:"), Doc::space(), right_doc]),
            ),
        ]),
    ))
}

use super::chain_step::{ChainStep, should_break_scoping_root, should_step_group_with_previous};

pub(crate) struct ChainAnalysis {
    pub(crate) has_comments: bool,
    pub(crate) break_scoping_root: bool,
    pub(crate) has_intermediate_lambda: bool,
    pub(crate) has_intermediate_break: bool,
    pub(crate) has_trailing_lambda: bool,
}

pub(crate) fn analyze_chain<'a>(
    steps: &[ChainStep<'a>],
    receiver: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> ChainAnalysis {
    let total_steps = steps.len();
    let has_comments = steps.iter().any(|s| !s.comments.is_empty());
    let is_scoping_chain = total_steps > 1
        && steps[0].lead_op.is_none()
        && steps[0]
            .call
            .as_ref()
            .is_some_and(|c| c.1.is_none() && c.2.is_some());
    let break_scoping_root = is_scoping_chain
        && steps[0].call.as_ref().is_some_and(|c| {
            c.2.as_ref()
                .is_some_and(|tl| should_break_scoping_root(receiver, tl, ctx))
        });
    let lambda_count = steps
        .iter()
        .filter(|s| s.call.as_ref().is_some_and(|c| c.2.is_some()))
        .count();
    let any_lambda_multiline = steps.iter().any(|s| {
        s.call
            .as_ref()
            .is_some_and(|c| c.2.is_some_and(super::chain_format::is_lambda_multiline))
    });
    let has_intermediate_lambda = break_scoping_root
        || (total_steps > 1
            && ((lambda_count > 1
                && (ctx.style.preset != crate::options::StylePreset::Kotlinlang
                    || any_lambda_multiline)
                && steps
                    .last()
                    .is_some_and(|s| s.call.as_ref().is_some_and(|c| c.2.is_some())))
                || steps[..total_steps - 1].iter().any(|s| {
                    s.call
                        .as_ref()
                        .is_some_and(|c| c.2.is_some_and(super::chain_format::is_lambda_multiline))
                })));
    let has_intermediate_break = has_intermediate_lambda
        || steps[..total_steps.saturating_sub(1)].iter().any(|s| {
            !s.comments.is_empty()
                || s.call
                    .as_ref()
                    .is_some_and(|c| c.1.is_some_and(|va| ctx.node_text(&va).contains('\n')))
        });
    let has_trailing_lambda = steps
        .last()
        .is_some_and(|s| s.call.as_ref().is_some_and(|c| c.2.is_some()));

    ChainAnalysis {
        has_comments,
        break_scoping_root,
        has_intermediate_lambda,
        has_intermediate_break,
        has_trailing_lambda,
    }
}

pub(crate) fn should_prefix_continue<'a>(
    step: &ChainStep<'a>,
    idx: usize,
    steps: &[ChainStep<'a>],
    receiver: &Node<'a>,
    prev_name: Option<&str>,
    prev_is_call: bool,
    prev_is_super_or_this: bool,
    prev_op: Option<&str>,
    has_trailing_lambda: bool,
    has_intermediate_break: bool,
    ctx: &LowerContext<'a, '_>,
) -> bool {
    let total_steps = steps.len();
    let total_calls = steps.iter().filter(|s| s.call.is_some()).count();
    let has_comments = steps.iter().any(|s| !s.comments.is_empty());
    let curr_target_name = step.lead_op.as_ref().map(|(_, _, t)| ctx.node_text(t));
    let curr_op = step.lead_op.as_ref().map(|(o, _, _)| *o);
    let curr_has_call = step.call.is_some();

    if matches!(
        receiver.kind(),
        "when_expression" | "if_expression" | "try_expression"
    ) && curr_has_call
    {
        return false;
    }
    if receiver.kind() == "callable_reference" {
        return false;
    }
    if idx == 0 && step.lead_op.is_none() {
        return true;
    }
    if !step.comments.is_empty() {
        return false;
    }
    if total_steps == 1 && has_trailing_lambda && !has_intermediate_break {
        let recv_len = prev_name.map(|n| n.len()).unwrap_or(0);
        let target_len = curr_target_name.map(|n| n.len()).unwrap_or(0);
        return recv_len + 1 + target_len + 2 <= ctx.options.max_width;
    }
    if total_calls <= 1 && has_trailing_lambda && !has_intermediate_break && !prev_is_call {
        return true;
    }
    if total_steps == 1
        && prev_name.is_some()
        && step
            .call
            .as_ref()
            .is_some_and(|c| c.1.is_none() && c.2.is_some())
        && curr_target_name.is_some_and(|t| {
            let recv_len = prev_name.map(|n| n.len()).unwrap_or(0);
            recv_len + 1 + t.len() + 2 <= ctx.options.max_width
        })
    {
        return true;
    }
    if total_steps == 1 && !curr_has_call && !has_comments {
        return true;
    }
    should_step_group_with_previous(
        idx,
        prev_is_call,
        prev_name,
        prev_is_super_or_this,
        prev_op,
        curr_op,
        curr_target_name,
        curr_has_call,
        ctx.options.continuation_indent,
    )
}

pub(crate) fn accumulate_chain_step<'a>(
    prefix_doc: &mut Doc<'a>,
    tail_docs: &mut Vec<Doc<'a>>,
    last_call_prefix_gid: &mut Option<crate::doc::GroupId>,
    selector_docs: Vec<Doc<'a>>,
    call_docs: Vec<Doc<'a>>,
    in_prefix: bool,
    is_last: bool,
    curr_has_call: bool,
    idx: usize,
    total_steps: usize,
    lead_op_is_none: bool,
    has_trailing_lambda: bool,
    has_intermediate_break: bool,
    receiver_kind: &str,
    step_call_is_none: bool,
    ctx: &LowerContext<'a, '_>,
) {
    if in_prefix {
        if (idx == 0 && lead_op_is_none)
            || (total_steps == 1 && has_trailing_lambda && !has_intermediate_break)
            || (total_steps == 1
                && !curr_has_call
                && matches!(
                    receiver_kind,
                    "when_expression" | "if_expression" | "try_expression" | "lambda_literal"
                ))
        {
            let prev = std::mem::replace(prefix_doc, Doc::empty());
            *prefix_doc = Doc::concat(vec![
                prev,
                Doc::concat(selector_docs),
                Doc::concat(call_docs),
            ]);
        } else if step_call_is_none {
            let mut step_docs = selector_docs;
            step_docs.extend(call_docs);
            let prev = std::mem::replace(prefix_doc, Doc::empty());
            *prefix_doc = Doc::group(Doc::concat(vec![
                prev,
                Doc::continuation_indent(Doc::concat(vec![Doc::SoftLine, Doc::concat(step_docs)])),
            ]));
        } else {
            let pgid = ctx.next_group_id();
            if is_last && curr_has_call {
                *last_call_prefix_gid = Some(pgid);
            }
            let prev = std::mem::replace(prefix_doc, Doc::empty());
            *prefix_doc = Doc::group_with_id(
                pgid,
                Doc::concat(vec![
                    prev,
                    Doc::continuation_indent(Doc::concat(vec![
                        Doc::SoftLine,
                        Doc::concat(selector_docs),
                    ])),
                ]),
            );
            if !call_docs.is_empty() {
                let prev_grouped = std::mem::replace(prefix_doc, Doc::empty());
                *prefix_doc = Doc::concat(vec![prev_grouped, Doc::concat(call_docs)]);
            }
        }
    } else {
        tail_docs.extend(selector_docs);
        tail_docs.extend(call_docs);
    }
}

pub fn format_last_call<'a>(
    chain_group: Doc<'a>,
    last_call: LastCallData<'a>,
    has_tail: bool,
    gid: crate::doc::GroupId,
    last_call_prefix_gid: Option<crate::doc::GroupId>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let (ta, va, tl) = last_call;
    let mut result = chain_group;
    let mut va_break_gid = None;
    let ta_gid = ctx.next_group_id();
    if let Some(ta_node) = ta {
        result = Doc::concat(vec![
            result,
            Doc::group_with_id(ta_gid, lower_fn(ta_node, ctx)),
        ]);
    }
    if let Some(va_node) = va {
        let v_gid = ctx.next_group_id();
        va_break_gid = Some((va_node, v_gid));
        let mut va_doc = lower_fn(va_node, ctx);
        if ta.is_some() {
            va_doc = Doc::if_group_break(
                ta_gid,
                super::params::lower_value_arguments_multiline(va_node, ctx, lower_fn),
                va_doc,
            );
        }
        if has_tail {
            va_doc = Doc::indent_if_group_breaks(gid, crate::doc::IndentKind::Continuation, va_doc);
        } else if let Some(pgid) = last_call_prefix_gid {
            va_doc =
                Doc::indent_if_group_breaks(pgid, crate::doc::IndentKind::Continuation, va_doc);
        }
        result = Doc::concat(vec![result, Doc::group_with_id(v_gid, va_doc)]);
    }
    if let Some(tl_node) = tl {
        let mut tl_doc = lower_fn(tl_node, ctx);
        if ctx.style.indent_trailing_lambda_after_broken_args
            && let Some((va_node, va_gid)) = va_break_gid
            && !ctx.has_trailing_comma(&va_node)
        {
            tl_doc =
                Doc::indent_if_group_breaks(va_gid, crate::doc::IndentKind::Continuation, tl_doc);
        }
        tl_doc = Doc::indent_if_group_breaks(gid, crate::doc::IndentKind::Continuation, tl_doc);
        result = Doc::concat(vec![result, Doc::space(), tl_doc]);
    }
    result
}
