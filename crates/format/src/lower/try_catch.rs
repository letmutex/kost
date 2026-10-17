use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_braced_children<'a>(children: &[Node<'a>], ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut stmt_docs = Vec::new();
    let mut prev: Option<Node<'a>> = None;
    for &child in children {
        if let Some(p) = prev {
            if ctx.has_blank_line_between(&p, &child) {
                stmt_docs.push(Doc::EmptyLine);
            } else if !ctx.has_newline_between(&p, &child) {
                stmt_docs.push(Doc::space());
            } else if !stmt_docs.last().is_some_and(|d| d.ends_with_newline()) {
                stmt_docs.push(Doc::HardLine);
            }
        }
        stmt_docs.push(lower_node(child, ctx));
        prev = Some(child);
    }
    if stmt_docs.is_empty() {
        Doc::text("{}")
    } else {
        Doc::Concat(vec![
            Doc::text("{"),
            Doc::indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(stmt_docs)])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    }
}

pub fn lower_catch_block<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut param_docs = Vec::new();
    let mut in_params = false;
    let mut in_body = false;
    let mut body_nodes = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" {
            in_params = true;
        } else if kind == ")" {
            in_params = false;
        } else if kind == "{" {
            in_body = true;
        } else if kind == "}" {
            in_body = false;
        } else if in_body {
            body_nodes.push(child);
        } else if in_params {
            if kind == ":" {
                param_docs.push(Doc::text(": "));
            } else if kind == "," {
                param_docs.push(Doc::text(","));
            } else {
                if !param_docs.is_empty()
                    && !param_docs
                        .last()
                        .is_some_and(|d| matches!(d, Doc::Text(": ")))
                {
                    param_docs.push(Doc::space());
                }
                param_docs.push(lower_node(child, ctx));
            }
        }
    }

    let body_doc = lower_braced_children(&body_nodes, ctx);
    Doc::Concat(vec![
        Doc::text("catch ("),
        Doc::Concat(param_docs),
        Doc::text(") "),
        body_doc,
    ])
}

pub fn lower_finally_block<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut in_body = false;
    let mut body_nodes = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" {
            in_body = true;
        } else if kind == "}" {
            in_body = false;
        } else if in_body {
            body_nodes.push(child);
        }
    }
    let body_doc = lower_braced_children(&body_nodes, ctx);
    Doc::Concat(vec![Doc::text("finally "), body_doc])
}

pub fn lower_try_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut try_body_nodes = Vec::new();
    let mut handlers = Vec::new();
    let mut in_try_body = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" && !in_try_body && try_body_nodes.is_empty() && handlers.is_empty() {
            in_try_body = true;
        } else if kind == "}" && in_try_body {
            in_try_body = false;
        } else if in_try_body {
            try_body_nodes.push(child);
        } else if kind == "catch_block" {
            handlers.push(lower_catch_block(child, ctx));
        } else if kind == "finally_block" {
            handlers.push(lower_finally_block(child, ctx));
        }
    }

    let try_body_doc = lower_braced_children(&try_body_nodes, ctx);
    let mut parts = vec![Doc::text("try "), try_body_doc];
    for h in handlers {
        parts.push(Doc::space());
        parts.push(h);
    }
    Doc::Concat(parts)
}
