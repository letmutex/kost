use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn find_leftmost_unparenthesized_elvis(node: Node<'_>) -> Option<Node<'_>> {
    let mut curr = node;
    loop {
        if curr.kind() == "parenthesized_expression" {
            return None;
        }
        if curr.kind() == "elvis_expression" {
            return Some(curr);
        }
        if matches!(
            curr.kind(),
            "call_expression"
                | "indexing_expression"
                | "navigation_expression"
                | "postfix_expression"
        ) {
            curr = curr.child(0)?;
        } else {
            return None;
        }
    }
}

pub fn lower_unparenthesized_elvis_chain<'a>(
    root: Node<'a>,
    elvis: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let lhs = elvis.child(0).unwrap();
    let op = elvis.child(1).unwrap();
    let rhs_leaf = elvis.child(2).unwrap();
    let has_newline = ctx.has_newline_between(&lhs, &op);
    let line_doc = if has_newline {
        Doc::HardLine
    } else {
        Doc::Line
    };
    let ctx_with_override = ctx.with_leaf_override(elvis.id(), rhs_leaf);
    let rhs_doc = lower_node(root, &ctx_with_override);
    Doc::Group(Box::new(Doc::Concat(vec![
        lower_node(lhs, ctx),
        Doc::continuation_indent(Doc::Concat(vec![line_doc, Doc::text("?: "), rhs_doc])),
    ])))
}
