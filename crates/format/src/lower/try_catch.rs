use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_braced_children<'a>(children: &[Node<'a>], ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut stmt_docs = Vec::new();
    let mut prev: Option<Node<'a>> = None;
    for &child in children {
        if let Some(prev_child) = prev {
            if ctx.has_blank_line_between(&prev_child, &child) {
                stmt_docs.push(Doc::EmptyLine);
            } else if !ctx.has_newline_between(&prev_child, &child) {
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

fn format_catch_parens<'a>(
    param_docs: Vec<Doc<'a>>,
    has_trailing_comma: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if param_docs.is_empty() {
        return Doc::text("()");
    }
    let trailing_comma_doc = match ctx.options.trailing_comma_strategy {
        crate::options::TrailingCommaStrategy::Complete => {
            Doc::if_break(Doc::text(","), Doc::empty())
        }
        crate::options::TrailingCommaStrategy::OnlyAdd => {
            if has_trailing_comma {
                Doc::text(",")
            } else {
                Doc::if_break(Doc::text(","), Doc::empty())
            }
        }
        crate::options::TrailingCommaStrategy::None => {
            if has_trailing_comma {
                Doc::text(",")
            } else {
                Doc::empty()
            }
        }
    };
    Doc::Concat(vec![
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text("("),
            Doc::continuation_indent(Doc::Concat(vec![
                Doc::SoftLine,
                Doc::Concat(param_docs),
                trailing_comma_doc,
                Doc::reserve_width(3),
            ])),
        ]))),
        Doc::text(")"),
    ])
}

pub fn lower_catch_block<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut param_docs = Vec::new();
    let mut in_params = false;
    let mut in_body = false;
    let mut body_nodes = Vec::new();
    let mut prev_param_child = None;

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
                // trailing comma handled by format_comma_list
            } else {
                if let Some(prev_child) = prev_param_child {
                    if ctx.has_newline_between(&prev_child, &child) {
                        if !param_docs.last().is_some_and(|d| d.ends_with_newline()) {
                            param_docs.push(Doc::HardLine);
                        }
                    } else if !param_docs.is_empty()
                        && !param_docs
                            .last()
                            .is_some_and(|d| matches!(d, Doc::Text(": ")) || d.ends_with_newline())
                    {
                        param_docs.push(Doc::space());
                    }
                }
                param_docs.push(lower_node(child, ctx));
                prev_param_child = Some(child);
            }
        }
    }

    let has_trailing_comma = node.children(&mut node.walk()).any(|c| c.kind() == ",");
    let paren_doc = format_catch_parens(param_docs, has_trailing_comma, ctx);

    let body_doc = lower_braced_children(&body_nodes, ctx);
    Doc::Concat(vec![Doc::text("catch "), paren_doc, Doc::space(), body_doc])
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
    let mut leading_comments = Vec::new();
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
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            leading_comments.push(lower_node(child, ctx));
        } else if kind == "catch_block" {
            handlers.push((
                std::mem::take(&mut leading_comments),
                lower_catch_block(child, ctx),
            ));
        } else if kind == "finally_block" {
            handlers.push((
                std::mem::take(&mut leading_comments),
                lower_finally_block(child, ctx),
            ));
        }
    }

    let try_body_doc = lower_braced_children(&try_body_nodes, ctx);
    let mut parts = vec![Doc::text("try "), try_body_doc];
    for (comments, handler) in handlers {
        if comments.is_empty() {
            parts.push(Doc::space());
        } else {
            for c in comments {
                parts.push(Doc::HardLine);
                parts.push(c);
            }
            parts.push(Doc::HardLine);
        }
        parts.push(handler);
    }
    Doc::Concat(parts)
}
