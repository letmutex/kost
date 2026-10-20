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
    let mut curr = *node;
    while curr.kind() == "postfix_expression" {
        if let Some(first) = curr.child(0) {
            curr = first;
        } else {
            break;
        }
    }
    matches!(
        curr.kind(),
        "call_expression" | "indexing_expression" | "parenthesized_expression"
    )
}

pub fn is_super_or_this(node: &Node<'_>) -> bool {
    matches!(node.kind(), "super_expression" | "this_expression")
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

pub fn has_call_in_receiver_chain(node: &Node<'_>) -> bool {
    let mut curr = *node;
    if curr.kind() == "call_expression" {
        let Some(callee) = curr.child(0) else {
            return false;
        };
        if callee.kind() == "navigation_expression" {
            let Some(first) = callee.child(0) else {
                return false;
            };
            curr = first;
        } else {
            return false;
        }
    } else if curr.kind() == "navigation_expression" {
        let Some(first) = curr.child(0) else {
            return false;
        };
        curr = first;
    } else {
        return false;
    }

    loop {
        while curr.kind() == "postfix_expression" {
            if let Some(first) = curr.child(0) {
                curr = first;
            } else {
                break;
            }
        }
        if matches!(
            curr.kind(),
            "call_expression"
                | "indexing_expression"
                | "parenthesized_expression"
                | "string_literal"
                | "multiline_string_literal"
        ) {
            return true;
        }
        if curr.kind() == "navigation_expression"
            && let Some(first) = curr.child(0)
        {
            curr = first;
            continue;
        }
        break;
    }
    false
}

pub fn is_part_of_call_chain(node: &Node<'_>) -> bool {
    let mut curr = *node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == "call_expression" {
            return true;
        }
        if parent.kind() == "navigation_expression" || parent.kind() == "postfix_expression" {
            curr = parent;
            continue;
        }
        break;
    }
    false
}

pub fn is_type_name_receiver(node: &Node<'_>, source: &str) -> bool {
    let mut curr = *node;
    while curr.kind() == "navigation_expression" {
        if let Some(last) = curr.child(curr.child_count().saturating_sub(1)) {
            curr = last;
        } else {
            break;
        }
    }
    let id_node = if curr.kind() == "navigation_suffix" {
        curr.children(&mut curr.walk())
            .find(|c| c.kind() == "simple_identifier")
    } else {
        Some(curr)
    };
    let name = match id_node.as_ref().map(|n| n.kind()) {
        Some("simple_identifier") => {
            let n = id_node.unwrap();
            source.get(n.start_byte()..n.end_byte())
        }
        _ => None,
    };
    name.is_some_and(|n| n.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
}
