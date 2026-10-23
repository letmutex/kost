use tree_sitter::Node;

use crate::lower::context::LowerContext;

#[derive(Clone)]
pub enum SpineSegment<'a> {
    MemberAccess {
        type_args: Option<Node<'a>>,
        op_token: &'a str,
        target: Node<'a>,
        comments: Vec<Node<'a>>,
        prev_end_byte: usize,
    },
    Call {
        type_args: Option<Node<'a>>,
        val_args: Option<Node<'a>>,
        trailing_lambda: Option<Node<'a>>,
    },
    Indexing {
        suffix_node: Node<'a>,
    },
    Postfix {
        op_token: &'a str,
    },
}

pub fn is_spine_root(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        "call_expression" | "navigation_expression" | "indexing_expression" | "postfix_expression"
    )
}

fn extract_call_segment<'a>(call_node: Node<'a>) -> SpineSegment<'a> {
    let mut type_args = None;
    let mut val_args = None;
    let mut trailing_lambda = None;

    let mut cursor = call_node.walk();
    for child in call_node.children(&mut cursor) {
        match child.kind() {
            "type_arguments" => type_args = Some(child),
            "value_arguments" => val_args = Some(child),
            "lambda_literal" | "annotated_lambda" => trailing_lambda = Some(child),
            "call_suffix" => {
                let mut sc = child.walk();
                for s_child in child.children(&mut sc) {
                    match s_child.kind() {
                        "type_arguments" => type_args = Some(s_child),
                        "value_arguments" => val_args = Some(s_child),
                        "lambda_literal" | "annotated_lambda" => trailing_lambda = Some(s_child),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    SpineSegment::Call {
        type_args,
        val_args,
        trailing_lambda,
    }
}

fn extract_spine_step<'a>(
    curr: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Option<(Node<'a>, SpineSegment<'a>)> {
    match curr.kind() {
        "navigation_expression" => {
            let mut recv = None;
            let mut comments = Vec::new();
            let mut suf = None;
            let mut cursor = curr.walk();
            for child in curr.children(&mut cursor) {
                if recv.is_none() {
                    recv = Some(child);
                } else if matches!(
                    child.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                ) {
                    comments.push(child);
                } else {
                    suf = Some(child);
                }
            }
            let recv = recv?;
            let suf = suf?;
            let prev_end_byte = recv.end_byte();
            let (type_args, op, tgt) = if suf.kind() == "navigation_suffix" {
                if suf.child(0).is_some_and(|c| c.kind() == "type_arguments") {
                    let ta = suf.child(0);
                    let op = suf.child(1).map(|c| ctx.node_text(&c)).unwrap_or(".");
                    let tgt = suf.child(2)?;
                    (ta, op, tgt)
                } else {
                    let op = suf.child(0).map(|c| ctx.node_text(&c)).unwrap_or(".");
                    let tgt = suf.child(1)?;
                    (None, op, tgt)
                }
            } else {
                let op = ctx.node_text(&suf);
                let tgt = curr.child(curr.child_count() - 1)?;
                (None, op, tgt)
            };
            Some((
                recv,
                SpineSegment::MemberAccess {
                    type_args,
                    op_token: op,
                    target: tgt,
                    comments,
                    prev_end_byte,
                },
            ))
        }
        "call_expression" => Some((curr.child(0)?, extract_call_segment(curr))),
        "indexing_expression" => Some((
            curr.child(0)?,
            SpineSegment::Indexing {
                suffix_node: curr.child(1)?,
            },
        )),
        "postfix_expression" => {
            let op = curr.child(1).map(|c| ctx.node_text(&c)).unwrap_or("!!");
            Some((curr.child(0)?, SpineSegment::Postfix { op_token: op }))
        }
        _ => None,
    }
}

pub fn flatten_spine<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> (Node<'a>, Vec<SpineSegment<'a>>) {
    let mut segments = Vec::new();
    let mut curr = node;

    while is_spine_root(&curr) {
        if let Some((next, seg)) = extract_spine_step(curr, ctx) {
            segments.push(seg);
            curr = next;
        } else {
            break;
        }
    }

    segments.reverse();
    (curr, segments)
}

#[derive(Default)]
pub(crate) struct ChainStep<'a> {
    pub comments: Vec<Node<'a>>,
    pub prev_end_byte: usize,
    pub lead_op: Option<(&'a str, Option<Node<'a>>, Node<'a>)>,
    pub call: Option<(Option<Node<'a>>, Option<Node<'a>>, Option<Node<'a>>)>,
    pub post_ops: Vec<PostOp<'a>>,
}

pub(crate) enum PostOp<'a> {
    Indexing(Node<'a>),
    Postfix(&'a str),
}

pub(crate) fn assemble_chain_steps<'a>(segments: Vec<SpineSegment<'a>>) -> Vec<ChainStep<'a>> {
    let mut steps = Vec::new();
    let mut current_step = ChainStep::default();

    for seg in segments {
        match seg {
            SpineSegment::MemberAccess {
                type_args,
                op_token,
                target,
                comments,
                prev_end_byte,
            } => {
                if current_step.lead_op.is_some()
                    || current_step.call.is_some()
                    || !current_step.post_ops.is_empty()
                {
                    steps.push(current_step);
                    current_step = ChainStep::default();
                }
                current_step.lead_op = Some((op_token, type_args, target));
                current_step.comments = comments;
                current_step.prev_end_byte = prev_end_byte;
            }
            SpineSegment::Call {
                type_args,
                val_args,
                trailing_lambda,
            } => {
                if current_step.call.is_some() || !current_step.post_ops.is_empty() {
                    steps.push(current_step);
                    current_step = ChainStep::default();
                }
                current_step.call = Some((type_args, val_args, trailing_lambda));
            }
            SpineSegment::Indexing { suffix_node } => {
                current_step.post_ops.push(PostOp::Indexing(suffix_node));
            }
            SpineSegment::Postfix { op_token } => {
                current_step.post_ops.push(PostOp::Postfix(op_token));
            }
        }
    }
    if current_step.lead_op.is_some()
        || current_step.call.is_some()
        || !current_step.post_ops.is_empty()
    {
        steps.push(current_step);
    }
    steps
}

pub(crate) fn should_step_group_with_previous(
    idx: usize,
    prev_is_call: bool,
    prev_name: Option<&str>,
    prev_is_super_or_this: bool,
    prev_op: Option<&str>,
    curr_op: Option<&str>,
    curr_target_name: Option<&str>,
    curr_has_call: bool,
    continuation_indent: usize,
) -> bool {
    if prev_op == Some("::") {
        return false;
    }
    if idx == 0
        && let Some(name) = prev_name
        && name.len() < continuation_indent
    {
        return true;
    }
    if prev_is_super_or_this {
        return true;
    }
    if !curr_has_call {
        if !prev_is_call && curr_op == Some(".") {
            return true;
        }
        if let Some(target) = curr_target_name
            && target.starts_with(|c: char| c.is_ascii_uppercase())
            && !target.chars().all(|c| !c.is_ascii_lowercase())
        {
            return true;
        }
        return false;
    }
    if !prev_is_call
        && let Some(name) = prev_name
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && !name.chars().all(|c| !c.is_ascii_lowercase())
    {
        return true;
    }
    false
}

fn compute_ast_indent_col(node: Node<'_>, block_indent: usize) -> usize {
    let mut depth = 0usize;
    let mut curr = node.parent();
    while let Some(p) = curr {
        if matches!(
            p.kind(),
            "class_body" | "enum_class_body" | "block" | "when_expression" | "lambda_literal"
        ) {
            depth += 1;
        }
        curr = p.parent();
    }
    depth * block_indent
}

pub(crate) fn should_break_scoping_root(
    receiver: &Node<'_>,
    tl: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let mut p = receiver.parent();
    let mut start_node = *receiver;
    let mut inside_binary = false;
    let mut on_continuation_line = false;
    while let Some(parent) = p {
        if matches!(
            parent.kind(),
            "equality_expression"
                | "comparison_expression"
                | "elvis_expression"
                | "conjunction_expression"
                | "disjunction_expression"
                | "additive_expression"
                | "multiplicative_expression"
                | "infix_expression"
        ) {
            inside_binary = true;
        }
        if parent
            .children(&mut parent.walk())
            .find(|c| c.kind() == "=")
            .is_some_and(|eq| eq.end_position().row != receiver.start_position().row)
        {
            let rhs_is_block = parent
                .child(parent.child_count().saturating_sub(1))
                .is_some_and(|rhs| crate::lower::builders::is_block_like_rhs(&rhs, ctx));
            if inside_binary || !rhs_is_block {
                on_continuation_line = true;
                break;
            }
        }
        if matches!(
            parent.kind(),
            "property_declaration" | "variable_declaration" | "assignment" | "function_declaration"
        ) {
            start_node = parent;
            break;
        }
        if matches!(
            parent.kind(),
            "statements"
                | "source_file"
                | "value_argument"
                | "value_arguments"
                | "parenthesized_expression"
                | "when_entry"
                | "lambda_literal"
        ) {
            break;
        }
        start_node = parent;
        p = parent.parent();
    }
    let start_byte = start_node.start_byte();
    let start_col = compute_ast_indent_col(start_node, ctx.options.block_indent)
        + if on_continuation_line {
            ctx.options.continuation_indent
        } else {
            0
        };
    let root_span = tl.end_byte().saturating_sub(start_byte);
    start_col + root_span >= ctx.options.max_width
}
