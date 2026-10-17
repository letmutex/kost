use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_while_statement<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut cond = None;
    let mut body = None;
    let mut comments = Vec::new();
    let mut has_semi = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ";" {
            has_semi = true;
            continue;
        }
        if kind == "while" || kind == "(" || kind == ")" {
            continue;
        }
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            comments.push(lower_node(child, ctx));
            continue;
        }
        if cond.is_none() {
            cond = Some(lower_node(child, ctx));
        } else {
            body = Some(lower_node(child, ctx));
        }
    }
    let cond_doc = cond.unwrap_or_else(Doc::empty);
    let while_head = if ctx.options.trailing_comma_strategy != crate::TrailingCommaStrategy::None {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text("while ("),
            Doc::indent(Doc::Concat(vec![Doc::SoftLine, cond_doc])),
            Doc::SoftLine,
            Doc::text(")"),
        ])))
    } else {
        Doc::Concat(vec![Doc::text("while ("), cond_doc, Doc::text(")")])
    };
    let mut parts = vec![while_head];
    if let Some(b) = body {
        parts.push(Doc::space());
        parts.push(b);
    } else if has_semi {
        if comments.is_empty() {
            parts.push(Doc::space());
            parts.push(Doc::text(";"));
        } else {
            for c in comments {
                parts.push(Doc::HardLine);
                parts.push(c);
            }
            parts.push(Doc::HardLine);
            parts.push(Doc::text(";"));
        }
    }
    Doc::Concat(parts)
}

pub fn lower_do_while_statement<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut body = None;
    let mut cond = None;
    let mut is_after_while = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "while" {
            is_after_while = true;
            continue;
        }
        if kind == "do" || kind == "(" || kind == ")" || kind == ";" {
            continue;
        }
        if is_after_while {
            cond = Some(lower_node(child, ctx));
        } else {
            body = Some(lower_node(child, ctx));
        }
    }
    let mut parts = vec![Doc::text("do")];
    if let Some(b) = body {
        parts.push(Doc::space());
        parts.push(b);
    }
    let cond_doc = cond.unwrap_or_else(Doc::empty);
    if ctx.options.trailing_comma_strategy != crate::TrailingCommaStrategy::None {
        parts.push(Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text(" while ("),
            Doc::indent(Doc::Concat(vec![Doc::SoftLine, cond_doc])),
            Doc::SoftLine,
            Doc::text(")"),
        ]))));
    } else {
        parts.push(Doc::text(" while ("));
        parts.push(cond_doc);
        parts.push(Doc::text(")"));
    }
    Doc::Concat(parts)
}

pub fn lower_for_statement<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut loop_vars = Vec::new();
    let mut iter_expr = None;
    let mut body = None;
    let mut is_after_in = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "for" || kind == "(" || kind == ")" || kind == ";" {
            continue;
        }
        if kind == "in" {
            is_after_in = true;
            continue;
        }
        if !is_after_in {
            loop_vars.push(lower_node(child, ctx));
        } else if iter_expr.is_none() {
            iter_expr = Some(lower_node(child, ctx));
        } else {
            body = Some(lower_node(child, ctx));
        }
    }

    let mut header = vec![Doc::text("for (")];
    for (i, v) in loop_vars.into_iter().enumerate() {
        if i > 0 {
            header.push(Doc::space());
        }
        header.push(v);
    }
    header.push(Doc::text(" in"));
    if let Some(it) = iter_expr {
        header.push(Doc::continuation_indent(Doc::Concat(vec![Doc::Line, it])));
    }
    header.push(Doc::text(")"));
    let mut parts = vec![Doc::Group(Box::new(Doc::Concat(header)))];
    if let Some(b) = body {
        parts.push(Doc::space());
        parts.push(b);
    }
    Doc::Concat(parts)
}
