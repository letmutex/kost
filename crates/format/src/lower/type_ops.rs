use tree_sitter::Node;

use crate::doc::Doc;
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
        } else if matches!(kind, "as" | "as?" | "is" | "!is" | "!") {
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
    for t in type_nodes {
        if !type_docs.is_empty()
            && !type_docs
                .last()
                .is_some_and(|d: &Doc<'_>| d.ends_with_space() || d.ends_with_line())
        {
            type_docs.push(Doc::space());
        }
        type_docs.push(lower_node(t, ctx));
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

pub fn lower_prefix_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut prev_child: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        if let Some(prev) = prev_child {
            let prev_text = ctx.node_text(&prev);
            let next_text = ctx.node_text(&child);
            let is_anno_or_label = node
                .child(0)
                .is_some_and(|c| c.kind() == "annotation" || c.kind() == "label");
            let needs_op_space = matches!(
                (prev_text.chars().last(), next_text.chars().next()),
                (Some('+'), Some('+')) | (Some('-'), Some('-')) | (Some('!'), Some('!'))
            );
            if is_anno_or_label
                && !docs
                    .last()
                    .is_some_and(|d: &Doc<'_>| d.ends_with_newline() || d.ends_with_space())
            {
                if ctx.has_newline_between(&prev, &child) {
                    docs.push(Doc::HardLine);
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
    Doc::Concat(docs)
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
