use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;
use crate::lower::navigation::is_navigation_chain_root;

pub fn lower_double_colon_navigation<'a>(
    node: &Node<'a>,
    op_doc: Doc<'a>,
    suf_node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut member = None;
    let mut cursor = suf_node.walk();
    for child in suf_node.children(&mut cursor) {
        if child.kind() != "::"
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            member = Some(child);
        }
    }
    let mem_doc = member
        .map(|m| lower_node(m, ctx))
        .unwrap_or_else(Doc::empty);
    let chain_doc = Doc::Concat(vec![
        op_doc,
        Doc::text("::"),
        Doc::continuation_indent(Doc::Concat(vec![Doc::SoftLine, mem_doc])),
    ]);
    if is_navigation_chain_root(node) {
        Doc::Group(Box::new(chain_doc))
    } else {
        chain_doc
    }
}

pub fn lower_callable_reference<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut receiver_docs = Vec::new();
    let mut op_text = "::";
    let mut member = None;
    let mut seen_colon = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "::" || kind == "?::" {
            op_text = ctx.node_text(&child);
            seen_colon = true;
        } else if !seen_colon {
            receiver_docs.push(lower_node(child, ctx));
        } else {
            member = Some(child);
        }
    }

    if !receiver_docs.is_empty()
        && let Some(mem) = member
    {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::Concat(receiver_docs),
            Doc::text(op_text),
            Doc::continuation_indent(Doc::Concat(vec![Doc::SoftLine, lower_node(mem, ctx)])),
        ])))
    } else if let Some(mem) = member {
        Doc::Concat(vec![Doc::text(op_text), lower_node(mem, ctx)])
    } else {
        Doc::text(ctx.node_text(&node))
    }
}
