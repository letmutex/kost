use tree_sitter::Node;

use crate::lower::context::LowerContext;

/// Disambiguates whether a call_expression is actually two merged statements across a newline.
///
/// Tree-sitter sometimes parses `foo()\n(bar)` as a single call_expression instead of
/// two separate statements.
pub fn find_merge_call<'a>(
    expr: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Option<(Node<'a>, Node<'a>, Node<'a>)> {
    let mut curr = expr;
    loop {
        if curr.kind() == "call_expression"
            && let Some(callee) = curr.child(0)
        {
            let is_callee_completed =
                is_completed_callee(callee) || ctx.has_blank_line_between(&callee, &curr);
            let mut cursor = curr.walk();
            for child in curr.children(&mut cursor) {
                if child.kind() == "call_suffix"
                    && child
                        .child(0)
                        .is_some_and(|c| c.kind() == "value_arguments")
                    && ctx.has_user_newline_between(&callee, &child)
                    && (is_callee_completed || ctx.has_blank_line_between(&callee, &child))
                {
                    return Some((curr, callee, child));
                }
            }
            curr = callee;
        } else if curr.kind() == "navigation_expression"
            && let Some(receiver) = curr.child(0)
        {
            curr = receiver;
        } else {
            break;
        }
    }
    None
}

fn is_completed_callee(callee: Node<'_>) -> bool {
    if matches!(
        callee.kind(),
        "lambda_literal" | "as_expression" | "check_expression"
    ) {
        return true;
    }
    if callee.kind() == "call_expression" {
        let mut cursor = callee.walk();
        for ch in callee.children(&mut cursor) {
            if ch.kind() == "call_suffix" {
                return true;
            }
        }
    }
    false
}

/// Identifies an orphan secondary constructor following a bodiless class declaration.
pub fn is_orphan_constructor(node: Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    (node.kind() == "call_expression" || node.kind() == "secondary_constructor")
        && ctx.node_text(&node).starts_with("constructor")
}

/// Identifies comments directly preceding an orphan constructor.
pub fn is_comment_before_orphan_constructor(comment: Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut next = comment.next_sibling();
    while let Some(n) = next {
        if matches!(n.kind(), "comment" | "line_comment" | "multiline_comment") {
            next = n.next_sibling();
        } else {
            return is_orphan_constructor(n, ctx);
        }
    }
    false
}
