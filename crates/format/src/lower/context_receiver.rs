use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

enum ContextEntry<'a> {
    Param {
        name: Option<Node<'a>>,
        modifiers: Vec<Node<'a>>,
        type_node: Node<'a>,
    },
    Comment(Node<'a>),
}

fn parse_context_entries<'a>(node: Node<'a>) -> (Vec<ContextEntry<'a>>, Option<Node<'a>>) {
    let mut cursor = node.walk();
    let mut items = Vec::new();
    let mut open_paren = None;
    let mut param_name = None;
    let mut modifiers = Vec::new();
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
            items.push(ContextEntry::Comment(child));
            continue;
        }
        if !is_after_colon
            && child.kind() == "simple_identifier"
            && child.next_sibling().is_some_and(|s| s.kind() == ":")
        {
            param_name = Some(child);
            continue;
        }
        if kind == "type_modifiers" {
            modifiers.push(child);
            continue;
        }
        items.push(ContextEntry::Param {
            name: param_name.take(),
            modifiers: std::mem::take(&mut modifiers),
            type_node: child,
        });
        is_after_colon = false;
    }

    (items, open_paren)
}

fn format_param_doc<'a>(
    name: Option<Node<'a>>,
    modifiers: Vec<Node<'a>>,
    type_node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Vec<Doc<'a>> {
    let mut docs = Vec::new();
    if let Some(n) = name {
        docs.push(Doc::text(ctx.node_text(&n)));
        docs.push(Doc::text(": "));
    }
    for m in modifiers {
        docs.push(lower_node(m, ctx));
    }
    docs.push(lower_node(type_node, ctx));
    docs
}

fn lower_multiline_context<'a>(
    items: Vec<ContextEntry<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut docs = vec![Doc::text("context("), Doc::HardLine];
    let total = items.len();
    for (idx, entry) in items.into_iter().enumerate() {
        let is_last = idx + 1 == total;
        match entry {
            ContextEntry::Comment(c) => {
                docs.push(lower_node(c, ctx));
                docs.push(Doc::HardLine);
            }
            ContextEntry::Param {
                name,
                modifiers,
                type_node,
            } => {
                docs.extend(format_param_doc(name, modifiers, type_node, ctx));
                if !is_last {
                    docs.push(Doc::text(","));
                    docs.push(Doc::HardLine);
                }
            }
        }
    }
    docs.push(Doc::text(")"));
    Doc::Concat(docs)
}

fn lower_single_line_context<'a>(
    items: Vec<ContextEntry<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut docs = vec![Doc::text("context(")];
    let total = items.len();
    for (idx, entry) in items.into_iter().enumerate() {
        let is_last = idx + 1 == total;
        match entry {
            ContextEntry::Comment(c) => {
                docs.push(lower_node(c, ctx));
                if !is_last {
                    docs.push(Doc::space());
                }
            }
            ContextEntry::Param {
                name,
                modifiers,
                type_node,
            } => {
                docs.extend(format_param_doc(name, modifiers, type_node, ctx));
                if !is_last {
                    docs.push(Doc::text(", "));
                }
            }
        }
    }
    docs.push(Doc::text(")"));
    Doc::Concat(docs)
}

pub fn lower_context_receiver<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let (items, open_paren) = parse_context_entries(node);
    let is_multiline = open_paren.is_some_and(|p| {
        items.first().is_some_and(|first| match first {
            ContextEntry::Param { name: Some(n), .. } => ctx.has_newline_between(&p, n),
            ContextEntry::Param {
                modifiers,
                type_node,
                ..
            } => {
                let first_node = modifiers.first().unwrap_or(type_node);
                ctx.has_newline_between(&p, first_node)
            }
            ContextEntry::Comment(c) => ctx.has_newline_between(&p, c),
        })
    });

    if is_multiline {
        lower_multiline_context(items, ctx)
    } else {
        lower_single_line_context(items, ctx)
    }
}
