use tree_sitter::Node;

use crate::lower::context::LowerContext;

pub fn is_navigation_chain_root(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return true;
    };
    if matches!(
        parent.kind(),
        "navigation_expression" | "postfix_expression"
    ) {
        return false;
    }
    if parent.kind() == "indexing_expression"
        && parent.child(0).is_some_and(|c| c.id() == node.id())
    {
        return false;
    }
    if parent.kind() == "call_expression"
        && let Some(grandparent) = parent.parent()
    {
        if matches!(
            grandparent.kind(),
            "navigation_expression" | "postfix_expression"
        ) {
            return false;
        }
        if grandparent.kind() == "indexing_expression"
            && grandparent
                .child(0)
                .is_some_and(|child| child.id() == parent.id())
        {
            return false;
        }
    }
    true
}

pub fn is_call_receiver(node: &Node<'_>) -> bool {
    matches!(node.kind(), "call_expression" | "indexing_expression")
}

pub fn is_super_or_this(node: &Node<'_>) -> bool {
    matches!(node.kind(), "super_expression" | "this_expression")
}

pub fn is_part_of_call_chain(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    parent.kind() == "navigation_expression"
}

pub fn is_type_name_receiver(node: &Node<'_>, source: &str) -> bool {
    let name = match node.kind() {
        "simple_identifier" => source.get(node.start_byte()..node.end_byte()),
        _ => None,
    };
    name.is_some_and(|n| n.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
}

pub fn count_chain_lambdas(node: &Node<'_>) -> usize {
    let mut count = 0;
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "call_expression" {
            let mut cursor = n.walk();
            for child in n
                .children(&mut cursor)
                .filter(|c| c.kind() == "call_suffix")
            {
                let mut suffix_cursor = child.walk();
                for grandchild in child.children(&mut suffix_cursor) {
                    if matches!(grandchild.kind(), "lambda_literal" | "annotated_lambda") {
                        count += 1;
                    }
                }
            }
            curr = n.child(0);
        } else if n.kind() == "navigation_expression" {
            curr = n.child(0);
        } else {
            break;
        }
    }
    count
}

pub fn has_multiline_trailing_lambda(node: &Node<'_>) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let mut cursor = node.walk();
    for child in node
        .children(&mut cursor)
        .filter(|c| c.kind() == "call_suffix")
    {
        let mut suffix_cursor = child.walk();
        for grandchild in child.children(&mut suffix_cursor) {
            if matches!(grandchild.kind(), "lambda_literal" | "annotated_lambda")
                && grandchild.start_position().row != grandchild.end_position().row
            {
                return true;
            }
        }
    }
    false
}

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

pub fn resolve_chain_start_col(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> usize {
    let mut curr = *node;
    while let Some(parent) = curr.parent() {
        match parent.kind() {
            "property_declaration" => {
                let (header_len, op_len) = compute_declaration_header_len(&parent, curr.id(), ctx);
                let first_col =
                    ctx.compute_target_indent(&parent) + header_len.saturating_sub(1) + op_len;
                return init_start_col(parent, curr, first_col, ctx);
            }
            "destructuring_declaration" => {
                let (header_len, op_len) = compute_declaration_header_len(&parent, curr.id(), ctx);
                let first_col =
                    ctx.compute_target_indent(&parent) + header_len.saturating_sub(1) + op_len;
                return init_start_col(parent, curr, first_col, ctx);
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
            "statements" | "block" => return ctx.compute_target_indent(&curr),
            _ => curr = parent,
        }
    }
    ctx.compute_target_indent(node)
}

pub fn is_chain_broken(node: &Node<'_>, ctx: &LowerContext<'_, '_>, chain_len: usize) -> bool {
    let start_col = resolve_chain_start_col(node, ctx);
    if start_col + chain_len > ctx.options.max_width {
        return true;
    }
    if count_chain_lambdas(node) > 1 {
        return true;
    }
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if n.kind() == "call_expression" {
            curr = n.child(0);
        } else if n.kind() == "navigation_expression" {
            let mut cursor = n.walk();
            let mut prev_child: Option<Node<'_>> = None;
            for child in n.children(&mut cursor) {
                if let Some(prev) = prev_child {
                    if ctx.has_newline_between(&prev, &child)
                        || matches!(
                            prev.kind(),
                            "comment" | "line_comment" | "multiline_comment"
                        )
                    {
                        return true;
                    }
                } else if has_multiline_trailing_lambda(&child) {
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

pub fn is_call_receiver_in_chain(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    if ctx.options.block_indent == 4 {
        return false;
    }
    let mut parent = node.parent();
    while let Some(parent_node) = parent {
        if parent_node.kind() == "call_suffix" || parent_node.kind() == "indexing_expression" {
            break;
        }
        if parent_node.kind() != "annotated_lambda" {
            return false;
        }
        parent = parent_node.parent();
    }
    let Some(suffix) = parent else {
        return false;
    };
    if suffix.kind() == "indexing_expression" {
        if suffix.child(0).is_some_and(|operand| {
            operand.kind() == "navigation_expression" && ctx.node_text(&operand).contains('\n')
        }) {
            return true;
        }
        return suffix
            .parent()
            .is_some_and(|p| p.kind() == "navigation_expression");
    }
    if suffix.kind() != "call_suffix" {
        return false;
    }
    let Some(call) = suffix.parent() else {
        return false;
    };
    if call.kind() != "call_expression" {
        return false;
    }
    let Some(parent) = call.parent() else {
        return false;
    };
    if parent.kind() != "navigation_expression" {
        return false;
    }
    if let Some(callee) = call.child(0)
        && callee.kind() == "navigation_expression"
    {
        let mut cursor = callee.walk();
        let mut children = callee.children(&mut cursor);
        if let (Some(op), Some(suf)) = (children.next(), children.next())
            && ctx.has_newline_between(&op, &suf)
        {
            return false;
        }
    }
    true
}

pub fn is_spine_root(node: &Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return true;
    };
    if matches!(
        parent.kind(),
        "call_expression" | "indexing_expression" | "navigation_expression" | "postfix_expression"
    ) && parent.child(0).is_some_and(|c| c.id() == node.id())
    {
        return false;
    }
    true
}
