use tree_sitter::Node;

use crate::lower::context::LowerContext;

pub use crate::lower::chain_lambda::{
    count_chain_lambdas, has_call_in_receiver_chain, has_multiline_trailing_lambda,
    is_call_receiver, is_call_receiver_in_chain, is_navigation_chain_root, is_part_of_call_chain,
    is_spine_root, is_super_or_this, is_type_name_receiver,
};

fn init_start_col(
    parent: Node<'_>,
    curr: Node<'_>,
    first_col: usize,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    let is_kotlinlang = ctx.options.block_indent == 4;
    let len = crate::lower::delegates::property_init_first_line_len(
        Some(curr),
        ctx.node_text(&curr).len(),
        is_kotlinlang,
        ctx,
    );
    if first_col + len <= ctx.options.max_width {
        first_col
    } else {
        ctx.compute_target_indent(&parent) + ctx.options.continuation_indent
    }
}

fn compute_declaration_header_len(
    parent: &Node<'_>,
    curr_id: usize,
    ctx: &LowerContext<'_, '_>,
) -> (usize, usize) {
    let mut header_len = 0;
    let mut op_len = 3;
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" || kind == "by" {
            op_len = kind.len() + 2;
            break;
        }
        if child.id() == curr_id {
            break;
        }
        if kind == "modifiers" {
            header_len += ctx.node_text(&child).lines().last().unwrap_or("").len() + 1;
        } else {
            header_len += ctx.node_text(&child).len() + 1;
        }
    }
    (header_len, op_len)
}

fn resolve_value_argument_start_col(
    parent: Node<'_>,
    ctx: &LowerContext<'_, '_>,
    chain_len: usize,
) -> usize {
    let base = ctx.compute_target_indent(&parent);
    let mut extra = 0;
    let mut curr_ancestor = parent.parent();
    while let Some(p) = curr_ancestor {
        if p.kind() == "call_expression" {
            if let Some(callee) = p.child(0)
                && (callee.kind() == "navigation_expression"
                    || crate::lower::chain::is_call_receiver(&p))
            {
                extra = ctx.options.continuation_indent;
            }
            break;
        }
        if matches!(p.kind(), "statements" | "function_body" | "class_body") {
            break;
        }
        curr_ancestor = p.parent();
    }
    let name_prefix_len = parent
        .child(0)
        .filter(|c| c.kind() == "value_argument_name")
        .map(|c| ctx.node_text(&c).len() + 3)
        .unwrap_or(0);
    let col = base + extra + ctx.options.continuation_indent;
    if name_prefix_len > 0 {
        if col + name_prefix_len + chain_len <= ctx.options.max_width {
            col + name_prefix_len
        } else {
            col
        }
    } else {
        col
    }
}

fn resolve_statements_start_col(
    curr: Node<'_>,
    parent: Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> usize {
    let base = ctx.compute_target_indent(&curr);
    let mut is_in_chain_lambda = false;
    if let Some(parent_block) = parent.parent()
        && matches!(parent_block.kind(), "lambda_literal" | "annotated_lambda")
    {
        let mut n = parent_block;
        while let Some(p) = n.parent() {
            if let Some(callee) = p.child(0)
                && callee.kind() == "navigation_expression"
                && let Some(receiver) = callee.child(0)
                && receiver.kind() == "call_expression"
                && crate::lower::chain::has_call_in_receiver_chain(&receiver)
            {
                is_in_chain_lambda = true;
                break;
            }
            if matches!(p.kind(), "statements" | "class_body") {
                break;
            }
            n = p;
        }
    }
    let extra = if is_in_chain_lambda {
        ctx.options.continuation_indent
    } else {
        0
    };
    base + extra
}

pub fn resolve_chain_start_col(
    node: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
    chain_len: usize,
) -> usize {
    let mut curr = *node;
    while let Some(parent) = curr.parent() {
        match parent.kind() {
            "value_argument" => return resolve_value_argument_start_col(parent, ctx, chain_len),
            "property_declaration" | "destructuring_declaration" => {
                let (header_len, op_len) = compute_declaration_header_len(&parent, curr.id(), ctx);
                let first_col =
                    ctx.compute_target_indent(&parent) + header_len.saturating_sub(1) + op_len;
                return init_start_col(parent, curr, first_col, ctx);
            }
            "property_delegate" => {
                if let Some(prop_decl) = parent.parent() {
                    let (header_len, _) =
                        compute_declaration_header_len(&prop_decl, parent.id(), ctx);
                    let first_col =
                        ctx.compute_target_indent(&prop_decl) + header_len.saturating_sub(1) + 4;
                    return init_start_col(prop_decl, curr, first_col, ctx);
                }
                curr = parent;
            }
            "assignment" => {
                let lhs_len = parent
                    .child(0)
                    .map(|c| ctx.node_text(&c).len())
                    .unwrap_or(0);
                let op_len = parent
                    .children(&mut parent.walk())
                    .find(|c| matches!(c.kind(), "=" | "+=" | "-=" | "*=" | "/=" | "%="))
                    .map(|c| c.kind().len())
                    .unwrap_or(1);
                let first_col = ctx.compute_target_indent(&parent) + lhs_len + 2 + op_len;
                return init_start_col(parent, curr, first_col, ctx);
            }
            "binary_expression" => {
                let lhs = parent.child(0);
                let lhs_len = lhs.map(|c| ctx.node_text(&c).len()).unwrap_or(0);
                let op_len = parent
                    .child(1)
                    .map(|c| ctx.node_text(&c).len())
                    .unwrap_or(1);
                let first_col = ctx.compute_target_indent(&parent) + lhs_len + 2 + op_len;
                return init_start_col(parent, curr, first_col, ctx);
            }
            "jump_expression" => {
                let op_len = parent
                    .child(0)
                    .map(|c| ctx.node_text(&c).len() + 1)
                    .unwrap_or(7);
                let first_col = ctx.compute_target_indent(&parent) + op_len;
                return init_start_col(parent, curr, first_col, ctx);
            }
            "function_body" => {
                let func = parent.parent().unwrap_or(parent);
                let eq_col = ctx.compute_target_indent(&func);
                let extra = if matches!(func.kind(), "getter" | "setter") {
                    ctx.options.block_indent
                } else {
                    0
                };
                return eq_col + extra + ctx.options.continuation_indent;
            }
            "function_declaration" => {
                let eq_col = ctx.compute_target_indent(&parent);
                return eq_col + ctx.options.continuation_indent;
            }
            "for_statement" => {
                return ctx.compute_target_indent(&parent) + ctx.options.continuation_indent;
            }
            "statements" | "block" => return resolve_statements_start_col(curr, parent, ctx),
            _ => curr = parent,
        }
    }
    ctx.compute_target_indent(node)
}

pub fn call_has_multiline_type_args(call: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut cursor = call.walk();
    for child in call.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut sc = child.walk();
            for suffix_child in child.children(&mut sc) {
                if suffix_child.kind() == "type_arguments" {
                    if suffix_child.start_position().row != suffix_child.end_position().row
                        || crate::lower::lists::has_trailing_comma(&suffix_child)
                    {
                        return true;
                    }
                    let start_col =
                        crate::lower::call_args::compute_call_continuation_indent(call, ctx);
                    let text = ctx
                        .source
                        .get(call.start_byte()..suffix_child.end_byte())
                        .unwrap_or("");
                    let total_len =
                        start_col + crate::lower::expression_body::collapsed_text_len(text);
                    return total_len > ctx.options.max_width;
                }
            }
        }
    }
    false
}

fn call_has_trailing_lambda(call: &Node<'_>) -> bool {
    let mut cursor = call.walk();
    for child in call.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            let mut sc = child.walk();
            for sub in child.children(&mut sc) {
                if matches!(sub.kind(), "lambda_literal" | "annotated_lambda") {
                    return true;
                }
            }
        }
    }
    false
}

pub fn find_node_trailing_comment_len(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> usize {
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if let Some(next) = n.next_sibling()
            && matches!(
                next.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
            && !ctx.has_newline_between(&n, &next)
        {
            let mut total_len = 1 + ctx.node_text(&next).len();
            let mut c_curr = next;
            while let Some(c_next) = c_curr.next_sibling() {
                if matches!(c_next.kind(), "comment" | "line_comment")
                    && c_next.start_position().row == c_curr.end_position().row + 1
                {
                    total_len += 1 + ctx.node_text(&c_next).len();
                    c_curr = c_next;
                } else {
                    break;
                }
            }
            return total_len;
        }
        if let Some(parent) = n.parent()
            && matches!(
                parent.kind(),
                "call_suffix" | "call_expression" | "function_body" | "getter" | "setter"
            )
        {
            curr = Some(parent);
            continue;
        }
        break;
    }
    0
}

pub fn is_chain_broken(node: &Node<'_>, ctx: &LowerContext<'_, '_>, chain_len: usize) -> bool {
    let start_col = resolve_chain_start_col(node, ctx, chain_len);
    let trailing_comment_len = find_node_trailing_comment_len(node, ctx);
    if start_col + chain_len + trailing_comment_len > ctx.options.max_width {
        return true;
    }
    if count_chain_lambdas(node) > 1 {
        return true;
    }
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "call_expression" {
            let multiline_break = !call_has_trailing_lambda(&n)
                && (crate::lower::delegates::call_has_multiline_args(&n, ctx)
                    || call_has_multiline_type_args(&n, ctx));
            if multiline_break && (n.id() != node.id() || has_call_in_receiver_chain(&n)) {
                return true;
            }
            curr = n.child(0);
        } else if n.kind() == "navigation_expression" {
            let mut cursor = n.walk();
            let mut prev_child: Option<Node<'_>> = None;
            for child in n.children(&mut cursor) {
                if let Some(prev) = prev_child
                    && matches!(
                        prev.kind(),
                        "comment" | "line_comment" | "multiline_comment"
                    )
                {
                    return true;
                }
                prev_child = Some(child);
            }
            curr = n.child(0);
        } else {
            break;
        }
    }
    false
}
