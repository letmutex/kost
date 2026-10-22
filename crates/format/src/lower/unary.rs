use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

fn is_binary_expression_kind(kind: &str) -> bool {
    matches!(
        kind,
        "binary_expression"
            | "additive_expression"
            | "multiplicative_expression"
            | "comparison_expression"
            | "equality_expression"
            | "conjunction_expression"
            | "disjunction_expression"
            | "infix_expression"
            | "elvis_expression"
    )
}

fn is_parent_block(node: &Node<'_>) -> bool {
    let mut curr = *node;
    while let Some(parent) = curr.parent() {
        let parent_kind = parent.kind();
        if matches!(parent_kind, "statements" | "block") {
            return true;
        }
        if matches!(
            parent_kind,
            "function_body"
                | "lambda_literal"
                | "control_structure_body"
                | "source_file"
                | "parenthesized_expression"
                | "value_argument"
        ) {
            return false;
        }
        curr = parent;
    }
    false
}

fn prefix_chain_has_bracket_annotation(node: &Node<'_>) -> bool {
    let mut root = *node;
    while let Some(parent) = root.parent() {
        if parent.kind() == "prefix_expression" {
            root = parent;
        } else {
            break;
        }
    }
    if let Some(prev) = root.prev_sibling()
        && prev.kind() == "annotation"
        && prev.children(&mut prev.walk()).any(|c| c.kind() == "[")
    {
        return true;
    }
    fn check_node(n: Node<'_>) -> bool {
        let mut cursor = n.walk();
        for child in n.children(&mut cursor) {
            if child.kind() == "annotation"
                && child.children(&mut child.walk()).any(|c| c.kind() == "[")
            {
                return true;
            }
            if child.kind() == "prefix_expression" && check_node(child) {
                return true;
            }
        }
        false
    }
    check_node(root)
}

fn prefix_chain_has_block_expr(node: &Node<'_>) -> bool {
    let mut curr = *node;
    loop {
        let mut cursor = curr.walk();
        let mut found_next = false;
        for child in curr.children(&mut cursor) {
            if matches!(child.kind(), "when_expression" | "object_literal") {
                return true;
            }
            if child.kind() == "prefix_expression" {
                curr = child;
                found_next = true;
                break;
            }
        }
        if !found_next {
            break;
        }
    }
    false
}

fn try_lower_annotated_binary<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Option<Doc<'a>> {
    if node
        .parent()
        .is_some_and(|p| p.kind() == "prefix_expression")
    {
        return None;
    }
    let mut curr = node;
    let mut prefix_docs = Vec::new();
    let mut last_anno: Option<Node<'a>> = None;

    loop {
        if curr.kind() != "prefix_expression" {
            break;
        }
        let first = curr.child(0)?;
        if !matches!(first.kind(), "annotation" | "label") {
            return None;
        }
        if first.kind() == "annotation"
            && first.children(&mut first.walk()).any(|c| c.kind() == "[")
        {
            return None;
        }
        let second = curr.child(1)?;
        if let Some(last) = last_anno {
            if ctx.has_user_newline_between(&last, &first) {
                prefix_docs.push(Doc::HardLine);
            } else {
                prefix_docs.push(Doc::space());
            }
        }
        prefix_docs.push(lower_fn(first, ctx));
        last_anno = Some(first);

        if second.kind() == "prefix_expression" {
            let next_first = second.child(0)?;
            if matches!(next_first.kind(), "annotation" | "label") {
                curr = second;
                continue;
            }
        }
        let last = last_anno?;
        if is_parent_block(&node) && ctx.has_user_newline_between(&last, &second) {
            return None;
        }
        if is_binary_expression_kind(second.kind()) {
            let prefix_doc = Doc::concat(prefix_docs);
            return Some(super::binary::lower_binary_expression_with_prefix(
                second,
                Some(prefix_doc),
                ctx,
                lower_fn,
            ));
        }
        return None;
    }
    None
}

pub fn lower_prefix_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if let Some(doc) = try_lower_annotated_binary(node, ctx, lower_fn) {
        return doc;
    }

    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut prev_child: Option<Node<'a>> = None;
    let has_bracket_in_chain = prefix_chain_has_bracket_annotation(&node);

    for child in node.children(&mut cursor) {
        if let Some(prev) = prev_child {
            let prev_text = ctx.node_text(&prev);
            let next_text = ctx.node_text(&child);
            let is_anno = prev.kind() == "annotation";
            let is_label = prev.kind() == "label";
            let needs_op_space = matches!(
                (prev_text.chars().last(), next_text.chars().next()),
                (Some('+'), Some('+')) | (Some('-'), Some('-')) | (Some('!'), Some('!'))
            );
            if (is_anno || is_label)
                && !docs
                    .last()
                    .is_some_and(|d: &Doc<'_>| d.ends_with_newline() || d.ends_with_space())
            {
                let is_jump = child.kind() == "jump_expression";
                let is_block_expr = matches!(child.kind(), "when_expression" | "object_literal");
                let is_binary = is_binary_expression_kind(child.kind());
                let in_block = is_parent_block(&node);
                let has_newline = ctx.has_user_newline_between(&prev, &child);
                let is_next_anno = child.kind() == "annotation"
                    || (child.kind() == "prefix_expression"
                        && child.child(0).is_some_and(|c| c.kind() == "annotation"));

                if matches!(
                    child.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                ) {
                    if ctx.has_user_newline_between(&prev, &child) {
                        docs.push(Doc::HardLine);
                    } else {
                        docs.push(Doc::space());
                    }
                } else if has_bracket_in_chain
                    || is_jump
                    || is_block_expr
                    || (is_binary && in_block && has_newline)
                    || (is_next_anno
                        && (has_newline
                            || (ctx.options.block_indent == 4
                                && prefix_chain_has_block_expr(&node))))
                {
                    docs.push(Doc::HardLine);
                } else if is_next_anno && !has_newline {
                    docs.push(Doc::space());
                } else if is_anno {
                    docs.push(Doc::Line);
                } else {
                    docs.push(Doc::space());
                }
            } else if needs_op_space {
                docs.push(Doc::space());
            }
        }
        docs.push(lower_fn(child, ctx));
        prev_child = Some(child);
    }
    let res = Doc::concat(docs);
    if res.has_hardline() {
        res
    } else {
        Doc::group(res)
    }
}

pub fn lower_postfix_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    for (i, &child) in children.iter().enumerate() {
        if i == 1 && children.len() > 1 {
            let prev = children[0];
            if prev.kind() == "postfix_expression"
                && let Some(inner_op) = prev.child(1)
            {
                let op1 = ctx.node_text(&inner_op);
                let op2 = ctx.node_text(&child);
                if let (Some(c1), Some(c2)) = (op1.chars().last(), op2.chars().next())
                    && c1 == c2
                {
                    docs.push(Doc::space());
                }
            }
        }
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
}
