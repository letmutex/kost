use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::ast_eval::{AstEvalContext, infer_expression_node};
use super::call::CallInfo;
use super::expression::infer_receiver_type_depth_traced;
use super::member::resolve_member_call_info_depth_traced;
use super::trace::Tracer;
use super::types::KotlinType;
use crate::indexer::WorkspaceIndex;

fn extract_infix_operands<'a>(node: Node<'a>) -> Option<(Node<'a>, Node<'a>, Node<'a>)> {
    let mut non_comments = Vec::with_capacity(3);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            non_comments.push(child);
        }
    }
    if non_comments.len() == 3 && non_comments[1].kind() == "simple_identifier" {
        Some((non_comments[0], non_comments[1], non_comments[2]))
    } else {
        None
    }
}

pub fn resolve_infix_expression(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Option<KotlinType> {
    if !clean.chars().any(char::is_whitespace) {
        return None;
    }
    let tree = crate::parser::parse_source(clean)?;
    let call_root = tree.root_node();
    let infix_node = find_top_level_infix(call_root)?;
    let (child0, child1, child2) = extract_infix_operands(infix_node)?;

    let left_text = clean.get(child0.start_byte()..child0.end_byte())?.trim();
    let op_name = clean.get(child1.start_byte()..child1.end_byte())?.trim();
    let right_text = clean.get(child2.start_byte()..child2.end_byte())?.trim();

    if left_text.is_empty() || op_name.is_empty() || right_text.is_empty() {
        return None;
    }

    let left_type = infer_receiver_type_depth_traced(
        index,
        current_url,
        left_text,
        pos,
        source,
        root,
        depth + 1,
        &mut Tracer::Noop,
    )?;

    let right_type = infer_receiver_type_depth_traced(
        index,
        current_url,
        right_text,
        pos,
        source,
        root,
        depth + 1,
        &mut Tracer::Noop,
    );

    let member_call = common::concat_str!(op_name, "(", right_text, ")");
    let call_info = CallInfo {
        callee: op_name,
        explicit_type_args: Vec::new(),
        value_args: vec![right_text],
    };
    let inferred_arg_types = [right_type];

    let mut tracer = Tracer::Noop;
    let inf_ctx = super::InferenceContext::new(
        index,
        current_url,
        pos,
        source,
        root,
        depth + 1,
        &mut tracer,
    );
    resolve_member_call_info_depth_traced(
        inf_ctx,
        &left_type,
        &member_call,
        &call_info,
        &inferred_arg_types,
    )
}

pub fn infer_infix_node(ctx: &mut AstEvalContext, node: Node) -> Option<KotlinType> {
    let (child0, child1, child2) = extract_infix_operands(node)?;
    let op_name = ctx.node_text(child1)?;
    let right_text = ctx.node_text(child2)?;

    let left_type = infer_expression_node(&mut ctx.sub(), child0);
    let right_type = infer_expression_node(&mut ctx.sub(), child2);

    let left_type = left_type.or_else(|| {
        let left_text = ctx.node_text(child0)?;
        super::qualification::match_qualified_class(ctx.index, left_text, ctx.tracer)
            .or_else(|| super::expression::resolve_single_receiver(ctx.ctx.sub(), left_text))
    })?;

    let right_type = right_type.or_else(|| {
        let right_text = ctx.node_text(child2)?;
        super::qualification::match_qualified_class(ctx.index, right_text, ctx.tracer)
            .or_else(|| super::expression::resolve_single_receiver(ctx.ctx.sub(), right_text))
    });

    let member_call = common::concat_str!(op_name, "(", right_text, ")");
    let call_info = CallInfo {
        callee: op_name,
        explicit_type_args: Vec::new(),
        value_args: vec![right_text],
    };
    let inferred_arg_types = [right_type];

    resolve_member_call_info_depth_traced(
        ctx.ctx.sub(),
        &left_type,
        &member_call,
        &call_info,
        &inferred_arg_types,
    )
}

fn find_top_level_infix(node: Node) -> Option<Node> {
    if node.kind() == "infix_expression" {
        return Some(node);
    }
    if matches!(
        node.kind(),
        "source_file" | "statements" | "parenthesized_expression" | "labeled_expression"
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_top_level_infix(child) {
                return Some(found);
            }
        }
    }
    None
}
