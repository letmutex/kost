use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub(crate) struct IfComments<'a> {
    pub(crate) before_cond: Vec<Doc<'a>>,
    pub(crate) after_cond: Vec<Doc<'a>>,
    pub(crate) before_cons: Vec<Doc<'a>>,
    pub(crate) between_cons_and_else: Vec<Doc<'a>>,
    pub(crate) between_else_and_alt: Vec<Doc<'a>>,
}

pub(crate) fn collect_if_comments<'a>(
    node: &Node<'a>,
    cond: Option<Node<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> IfComments<'a> {
    let mut before_cond = Vec::new();
    let mut after_cond = Vec::new();
    let mut before_cons = Vec::new();
    let mut between_cons_and_else = Vec::new();
    let mut between_else_and_alt = Vec::new();

    let mut state = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" && state == 0 {
            state = 1;
        } else if Some(child) == cond && state <= 1 {
            state = 2;
        } else if kind == ")" && state <= 2 {
            state = 3;
        } else if Some(child) == consequence {
            state = 4;
        } else if kind == "else" {
            state = 5;
        } else if Some(child) == alternative {
            state = 6;
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            match state {
                1 => before_cond.push(lower_node(child, ctx)),
                2 => after_cond.push(lower_node(child, ctx)),
                3 => before_cons.push(lower_node(child, ctx)),
                4 => between_cons_and_else.push(lower_node(child, ctx)),
                5 => between_else_and_alt.push(lower_node(child, ctx)),
                _ => {}
            }
        }
    }
    IfComments {
        before_cond,
        after_cond,
        before_cons,
        between_cons_and_else,
        between_else_and_alt,
    }
}

pub(crate) fn lower_if_with_comments<'a>(
    parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    comments: &IfComments<'a>,
    has_empty_semi: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut out = parts;
    if let Some(cons) = consequence {
        let mut cons_parts = Vec::new();
        for c in &comments.before_cons {
            cons_parts.push(c.clone());
            if !cons_parts.last().is_some_and(|d| d.ends_with_newline()) {
                cons_parts.push(Doc::HardLine);
            }
        }
        cons_parts.push(lower_node(cons, ctx));
        out.push(Doc::continuation_indent(Doc::Concat(vec![
            Doc::HardLine,
            Doc::Concat(cons_parts),
        ])));
    } else if has_empty_semi {
        for c in &comments.before_cons {
            out.push(Doc::HardLine);
            out.push(c.clone());
        }
        out.push(Doc::HardLine);
        out.push(Doc::space());
        out.push(Doc::text(";"));
    }
    for c in &comments.between_cons_and_else {
        out.push(Doc::HardLine);
        out.push(c.clone());
    }
    if let Some(alt) = alternative {
        out.push(Doc::HardLine);
        out.push(Doc::text("else"));
        let mut alt_parts = Vec::new();
        for c in &comments.between_else_and_alt {
            alt_parts.push(Doc::HardLine);
            alt_parts.push(c.clone());
        }
        alt_parts.push(Doc::HardLine);
        alt_parts.push(lower_node(alt, ctx));
        out.push(Doc::continuation_indent(Doc::Concat(alt_parts)));
    }
    Doc::Concat(out)
}

pub(crate) fn format_else_with_comments<'a>(
    comments_between_else_and_alt: &[Doc<'a>],
    alternative: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut alt_parts = Vec::new();
    for c in comments_between_else_and_alt {
        alt_parts.push(Doc::HardLine);
        alt_parts.push(c.clone());
    }
    alt_parts.push(Doc::HardLine);
    if let Some(alt) = alternative {
        alt_parts.push(lower_node(alt, ctx));
    }
    Doc::continuation_indent(Doc::Concat(alt_parts))
}

pub(crate) fn lower_comments_before_else<'a>(
    comments_between_cons_and_else: &[Doc<'a>],
    comments_between_else_and_alt: &[Doc<'a>],
    alternative: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Vec<Doc<'a>> {
    let mut parts = Vec::new();
    for c in comments_between_cons_and_else {
        parts.push(Doc::HardLine);
        parts.push(c.clone());
    }
    parts.push(Doc::HardLine);
    parts.push(Doc::text("else"));
    if !comments_between_else_and_alt.is_empty() {
        parts.push(format_else_with_comments(
            comments_between_else_and_alt,
            alternative,
            ctx,
        ));
    } else if let Some(if_node) = alternative.and_then(super::if_expr::extract_single_if_child) {
        parts.push(Doc::space());
        parts.push(lower_node(if_node, ctx));
    } else if let Some(alt) = alternative {
        if super::if_expr::is_control_statement(alt) {
            parts.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_node(alt, ctx),
            ])));
        } else {
            parts.push(Doc::space());
            parts.push(lower_node(alt, ctx));
        }
    }
    parts
}

pub(crate) fn build_if_header<'a>(
    cond_doc: Doc<'a>,
    comments: &IfComments<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if !comments.before_cond.is_empty() {
        let mut before_cond_docs = Vec::new();
        for c in &comments.before_cond {
            before_cond_docs.push(Doc::space());
            before_cond_docs.push(c.clone());
        }
        let mut cond_parts = vec![cond_doc];
        for c in &comments.after_cond {
            cond_parts.push(Doc::space());
            cond_parts.push(c.clone());
        }
        Doc::Concat(vec![
            Doc::text("if ("),
            Doc::Concat(before_cond_docs),
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(cond_parts)])),
            Doc::HardLine,
            Doc::text(")"),
        ])
    } else if !comments.after_cond.is_empty() {
        let mut cond_parts = vec![cond_doc];
        for c in &comments.after_cond {
            cond_parts.push(Doc::space());
            cond_parts.push(c.clone());
        }
        Doc::Concat(vec![
            Doc::text("if ("),
            Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(cond_parts)])),
            Doc::HardLine,
            Doc::text(")"),
        ])
    } else if ctx.options.trailing_comma_strategy != crate::TrailingCommaStrategy::None {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text("if ("),
            Doc::continuation_indent(Doc::Concat(vec![Doc::SoftLine, cond_doc])),
            Doc::SoftLine,
            Doc::text(")"),
        ])))
    } else {
        Doc::Concat(vec![Doc::text("if ("), cond_doc, Doc::text(")")])
    }
}
