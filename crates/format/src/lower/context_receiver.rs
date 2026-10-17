use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_context_receiver<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut cursor = node.walk();
    let mut items: Vec<(Option<Node<'a>>, Node<'a>)> = Vec::new();
    let mut open_paren = None;
    let mut param_name = None;
    let mut is_after_colon = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" {
            open_paren = Some(child);
            continue;
        }
        if kind == ")" || kind == "context" || kind == "," {
            continue;
        }
        if kind == ":" {
            is_after_colon = true;
            continue;
        }
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            items.push((None, child));
            continue;
        }
        if !is_after_colon
            && child.kind() == "simple_identifier"
            && child.next_sibling().is_some_and(|s| s.kind() == ":")
        {
            param_name = Some(child);
            continue;
        }
        items.push((param_name.take(), child));
        is_after_colon = false;
    }

    let is_multiline = open_paren.is_some_and(|p| {
        items
            .first()
            .is_some_and(|(_, first)| ctx.has_newline_between(&p, first))
    });

    if is_multiline {
        let mut docs = vec![Doc::text("context("), Doc::HardLine];
        let total = items.len();
        for (idx, (name, item)) in items.into_iter().enumerate() {
            let is_last = idx + 1 == total;
            let is_comment = matches!(
                item.kind(),
                "comment" | "line_comment" | "multiline_comment"
            );
            if let Some(n) = name {
                docs.push(Doc::text(ctx.node_text(&n)));
                docs.push(Doc::text(": "));
            }
            docs.push(lower_node(item, ctx));
            if !is_comment && !is_last {
                docs.push(Doc::text(","));
                docs.push(Doc::HardLine);
            }
        }
        docs.push(Doc::text(")"));
        Doc::Concat(docs)
    } else {
        let mut docs = vec![Doc::text("context(")];
        let total = items.len();
        for (idx, (name, item)) in items.into_iter().enumerate() {
            let is_last = idx + 1 == total;
            if let Some(n) = name {
                docs.push(Doc::text(ctx.node_text(&n)));
                docs.push(Doc::text(": "));
            }
            docs.push(lower_node(item, ctx));
            if !is_last {
                docs.push(Doc::text(", "));
            }
        }
        docs.push(Doc::text(")"));
        Doc::Concat(docs)
    }
}
