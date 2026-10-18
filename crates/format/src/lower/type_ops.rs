use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::binary::is_binary_expression_kind;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_as_or_check_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut cursor = node.walk();
    let mut left = None;
    let mut op_tokens = Vec::new();
    let mut type_nodes = Vec::new();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if left.is_none() {
            left = Some(child);
        } else if matches!(kind, "as" | "as?" | "is" | "!is" | "!" | "in" | "!in") {
            op_tokens.push(ctx.node_text(&child));
        } else {
            type_nodes.push(child);
        }
    }

    let Some(left_node) = left else {
        return Doc::empty();
    };
    let op_text = op_tokens.join("");

    let is_nav = left_node.kind() == "navigation_expression";
    let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
    let break_before_op = matches!(node.kind(), "as_expression")
        || matches!(parent_kind, "value_argument" | "parenthesized_expression");

    let op_doc = if break_before_op {
        Doc::continuation_indent(Doc::Concat(vec![Doc::Line, Doc::smol(op_text)]))
    } else {
        Doc::Concat(vec![Doc::space(), Doc::smol(op_text)])
    };

    let mut type_docs = Vec::new();
    for type_node in type_nodes {
        if !type_docs.is_empty()
            && !type_docs
                .last()
                .is_some_and(|d: &Doc<'_>| d.ends_with_space() || d.ends_with_line())
        {
            type_docs.push(Doc::space());
        }
        type_docs.push(lower_node(type_node, ctx));
    }
    let type_doc = Doc::Concat(type_docs);

    let type_group = Doc::Group(Box::new(Doc::continuation_indent(Doc::Concat(vec![
        Doc::Line,
        type_doc,
    ]))));

    if is_nav {
        Doc::Concat(vec![
            lower_node(left_node, ctx),
            Doc::Group(Box::new(Doc::Concat(vec![op_doc, type_group]))),
        ])
    } else {
        Doc::Group(Box::new(Doc::Concat(vec![
            lower_node(left_node, ctx),
            op_doc,
            type_group,
        ])))
    }
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
            if child.kind() == "annotation" {
                if child.children(&mut child.walk()).any(|c| c.kind() == "[") {
                    return true;
                }
            } else if child.kind() == "prefix_expression" && check_node(child) {
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

fn try_lower_annotated_binary<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Option<Doc<'a>> {
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
            if ctx.has_newline_between(&last, &first) {
                prefix_docs.push(Doc::HardLine);
            } else {
                prefix_docs.push(Doc::space());
            }
        }
        prefix_docs.push(lower_node(first, ctx));
        last_anno = Some(first);

        if second.kind() == "prefix_expression" {
            let next_first = second.child(0)?;
            if matches!(next_first.kind(), "annotation" | "label") {
                curr = second;
                continue;
            }
        }
        let last = last_anno?;
        if is_parent_block(&node) && ctx.has_newline_between(&last, &second) {
            return None;
        }
        if is_binary_expression_kind(second.kind()) {
            let prefix_doc = Doc::Concat(prefix_docs);
            return Some(crate::lower::binary::lower_binary_expression_with_prefix(
                second,
                Some(prefix_doc),
                ctx,
            ));
        }
        return None;
    }
    None
}

pub fn lower_prefix_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    if let Some(doc) = try_lower_annotated_binary(node, ctx) {
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
                let has_newline = ctx.has_newline_between(&prev, &child);
                let is_next_anno = child.kind() == "annotation"
                    || (child.kind() == "prefix_expression"
                        && child.child(0).is_some_and(|c| c.kind() == "annotation"));

                if matches!(
                    child.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                ) {
                    docs.push(Doc::space());
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
        docs.push(lower_node(child, ctx));
        prev_child = Some(child);
    }
    let res = Doc::Concat(docs);
    if res.has_hardline() {
        res
    } else {
        Doc::Group(Box::new(res))
    }
}

pub fn lower_postfix_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut prev_child: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        if let Some(prev) = prev_child {
            let prev_text = ctx.node_text(&prev);
            let next_text = ctx.node_text(&child);
            let needs_op_space = matches!(
                (prev_text.chars().last(), next_text.chars().next()),
                (Some('+'), Some('+')) | (Some('-'), Some('-')) | (Some('!'), Some('!'))
            );
            if needs_op_space {
                docs.push(Doc::space());
            }
        }
        docs.push(lower_node(child, ctx));
        prev_child = Some(child);
    }
    Doc::Concat(docs)
}

pub fn lower_indexing_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_node(child, ctx));
    }
    let res = Doc::Concat(docs);
    if crate::lower::navigation::is_navigation_chain_root(&node) {
        Doc::Group(Box::new(res))
    } else {
        res
    }
}
