use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

fn is_inside_when_entry(node: &Node<'_>) -> bool {
    let mut curr = node.parent();
    while let Some(p) = curr {
        if p.kind() == "when_entry" {
            return true;
        }
        if matches!(
            p.kind(),
            "block" | "statements" | "source_file" | "class_body"
        ) {
            return false;
        }
        curr = p.parent();
    }
    false
}

#[derive(Default)]
struct IfParts<'a> {
    open_paren: Option<Node<'a>>,
    close_paren: Option<Node<'a>>,
    cond_node: Option<Node<'a>>,
    before_cond: Vec<Node<'a>>,
    after_cond: Vec<Node<'a>>,
    then_node: Option<Node<'a>>,
    before_cons: Vec<Node<'a>>,
    between_cons_and_else: Vec<Node<'a>>,
    else_token: Option<Node<'a>>,
    between_else_and_alt: Vec<Node<'a>>,
    else_node: Option<Node<'a>>,
    has_then_semi: bool,
    has_else_semi: bool,
}

fn parse_if_parts<'a>(node: Node<'a>) -> IfParts<'a> {
    let mut parts = IfParts::default();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "if" => {}
            "(" => parts.open_paren = Some(child),
            ")" => parts.close_paren = Some(child),
            "else" => parts.else_token = Some(child),
            ";" => {
                if parts.close_paren.is_some()
                    && parts.else_token.is_none()
                    && parts.then_node.is_none()
                {
                    parts.has_then_semi = true;
                } else if parts.else_token.is_some() && parts.else_node.is_none() {
                    parts.has_else_semi = true;
                }
            }
            "comment" | "line_comment" | "multiline_comment" => {
                if parts.close_paren.is_none() {
                    if parts.cond_node.is_none() {
                        parts.before_cond.push(child);
                    } else {
                        parts.after_cond.push(child);
                    }
                } else if parts.else_token.is_none() {
                    if parts.then_node.is_none() {
                        parts.before_cons.push(child);
                    } else {
                        parts.between_cons_and_else.push(child);
                    }
                } else if parts.else_node.is_none() {
                    parts.between_else_and_alt.push(child);
                }
            }
            _ => {
                if parts.close_paren.is_none() && parts.cond_node.is_none() {
                    parts.cond_node = Some(child);
                } else if parts.close_paren.is_some()
                    && parts.else_token.is_none()
                    && parts.then_node.is_none()
                {
                    parts.then_node = Some(child);
                } else if parts.else_token.is_some() && parts.else_node.is_none() {
                    parts.else_node = Some(child);
                }
            }
        }
    }
    parts
}

fn format_comment_bucket<'a>(
    comments: &[Node<'a>],
    anchor: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut parts = Vec::new();
    let mut prev = anchor;
    for c in comments {
        let is_same = prev.is_some_and(|p| !ctx.has_user_newline_between(&p, c));
        if is_same && c.kind() == "line_comment" {
            parts.push(Doc::line_comment(ctx.node_text(c), true, false));
        } else {
            parts.push(if is_same { Doc::space() } else { Doc::HardLine });
            parts.push(lower_fn(*c, ctx));
        }
        prev = Some(*c);
    }
    Doc::concat(parts)
}

fn format_if_header<'a>(
    parts: &IfParts<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if let Some(first_comment) = parts.before_cond.first()
        && parts
            .open_paren
            .is_some_and(|op| !ctx.has_user_newline_between(&op, first_comment))
    {
        let before_doc = format_comment_bucket(&parts.before_cond, parts.open_paren, ctx, lower_fn);
        let cond_only = parts
            .cond_node
            .map(|c| lower_fn(c, ctx))
            .unwrap_or(Doc::empty());
        let after_doc = format_comment_bucket(&parts.after_cond, parts.cond_node, ctx, lower_fn);
        return Doc::concat(vec![
            Doc::text("if ("),
            before_doc,
            Doc::HardLine,
            cond_only,
            after_doc,
            Doc::text(")"),
        ]);
    }

    let mut cond_parts = Vec::new();
    if !parts.before_cond.is_empty() {
        cond_parts.push(format_comment_bucket(
            &parts.before_cond,
            parts.open_paren,
            ctx,
            lower_fn,
        ));
        cond_parts.push(Doc::HardLine);
    }
    if let Some(cond) = parts.cond_node {
        cond_parts.push(lower_fn(cond, ctx));
    }
    let cond_doc = Doc::concat(cond_parts);
    let after_cond_doc = format_comment_bucket(&parts.after_cond, parts.cond_node, ctx, lower_fn);

    let cond_starts_with_anno = parts.cond_node.is_some_and(|c| {
        c.kind() == "prefix_expression" && c.child(0).is_some_and(|ch| ch.kind() == "annotation")
    });

    if ctx.style.soft_break_inside_condition_parens || cond_starts_with_anno {
        Doc::group(Doc::concat(vec![
            Doc::text("if ("),
            Doc::continuation_indent(Doc::concat(vec![Doc::SoftLine, cond_doc])),
            after_cond_doc,
            Doc::SoftLine,
            Doc::text(")"),
        ]))
    } else {
        Doc::concat(vec![
            Doc::text("if ("),
            cond_doc,
            after_cond_doc,
            Doc::text(")"),
        ])
    }
}

fn format_if_then<'a>(
    parts: &IfParts<'a>,
    header_doc: Doc<'a>,
    parent_node: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> (Doc<'a>, bool) {
    let then_gid = ctx.next_group_id();
    let mut then_is_braced = false;
    let then_part = if let Some(then_b) = parts.then_node {
        then_is_braced = is_braced_body(&then_b);
        let after_cond_wraps = parts
            .after_cond
            .iter()
            .any(|c| c.byte_range().len() + 20 > ctx.options.max_width);
        if then_is_braced || (after_cond_wraps && parts.else_node.is_some()) {
            let mut inner = vec![header_doc];
            for c in &parts.before_cons {
                inner.push(Doc::space());
                inner.push(lower_fn(*c, ctx));
            }
            inner.push(Doc::space());
            inner.push(lower_fn(then_b, ctx));
            Doc::concat(inner)
        } else if !parts.before_cons.is_empty() {
            let mut inner = Vec::new();
            for c in &parts.before_cons {
                inner.push(Doc::HardLine);
                inner.push(lower_fn(*c, ctx));
            }
            inner.push(Doc::HardLine);
            inner.push(lower_fn(then_b, ctx));
            Doc::concat(vec![
                header_doc,
                Doc::continuation_indent(Doc::concat(inner)),
            ])
        } else if is_inside_when_entry(parent_node) {
            Doc::concat(vec![header_doc, Doc::space(), lower_fn(then_b, ctx)])
        } else {
            Doc::group_with_id(
                then_gid,
                Doc::concat(vec![
                    header_doc,
                    Doc::indent_if_group_breaks(
                        then_gid,
                        crate::doc::IndentKind::Continuation,
                        Doc::concat(vec![Doc::Line, lower_fn(then_b, ctx)]),
                    ),
                ]),
            )
        }
    } else if parts.has_then_semi {
        if parts.else_node.is_some() || parts.has_else_semi {
            Doc::concat(vec![header_doc, Doc::space()])
        } else {
            let mut inner = vec![header_doc];
            for c in &parts.before_cons {
                inner.push(Doc::HardLine);
                inner.push(lower_fn(*c, ctx));
            }
            if !parts.before_cons.is_empty() {
                inner.push(Doc::HardLine);
            }
            inner.push(Doc::space());
            inner.push(Doc::text(";"));
            Doc::concat(inner)
        }
    } else if parts.else_node.is_some() || parts.has_else_semi {
        Doc::concat(vec![header_doc, Doc::space()])
    } else {
        header_doc
    };
    (then_part, then_is_braced)
}

fn format_if_else<'a>(
    parts: &IfParts<'a>,
    parent_node: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Option<Doc<'a>> {
    let else_b = parts.else_node?;
    let is_else_if = is_else_if_node(&else_b);
    let else_is_braced = is_braced_body(&else_b);

    if !parts.between_else_and_alt.is_empty() {
        let mut inner = Vec::new();
        for c in &parts.between_else_and_alt {
            inner.push(Doc::HardLine);
            inner.push(lower_fn(*c, ctx));
        }
        inner.push(Doc::HardLine);
        inner.push(lower_fn(else_b, ctx));
        let ind = if ctx.options.block_indent == 4 {
            Doc::concat(inner)
        } else {
            Doc::continuation_indent(Doc::concat(inner))
        };
        Some(Doc::concat(vec![Doc::text("else"), ind]))
    } else if is_else_if || else_is_braced || is_inside_when_entry(parent_node) {
        Some(Doc::concat(vec![Doc::text("else "), lower_fn(else_b, ctx)]))
    } else {
        let else_gid = ctx.next_group_id();
        Some(Doc::group_with_id(
            else_gid,
            Doc::concat(vec![
                Doc::text("else"),
                Doc::indent_if_group_breaks(
                    else_gid,
                    crate::doc::IndentKind::Continuation,
                    Doc::concat(vec![Doc::Line, lower_fn(else_b, ctx)]),
                ),
            ]),
        ))
    }
}

pub fn lower_if_expression<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let parts = parse_if_parts(node);
    let header_doc = format_if_header(&parts, ctx, lower_fn);
    let (then_part, then_is_braced) = format_if_then(&parts, header_doc, &node, ctx, lower_fn);

    let mut docs = vec![then_part];
    for c in &parts.between_cons_and_else {
        docs.push(Doc::HardLine);
        docs.push(lower_fn(*c, ctx));
    }

    if let Some(else_doc) = format_if_else(&parts, &node, ctx, lower_fn) {
        if !parts.between_cons_and_else.is_empty() {
            docs.push(Doc::HardLine);
        } else if then_is_braced {
            docs.push(Doc::space());
        } else {
            docs.push(Doc::Line);
        }
        docs.push(else_doc);
    } else if parts.has_else_semi {
        if then_is_braced {
            docs.push(Doc::space());
        } else {
            docs.push(Doc::Line);
        }
        docs.push(Doc::text("else ;"));
    }

    Doc::group(Doc::concat(docs))
}

pub use super::control_utils::{
    is_braced_body, is_else_if_node, lower_control_structure_body, lower_jump_expression,
    lower_type_or_range_test,
};
