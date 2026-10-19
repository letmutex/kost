use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::if_comments::{
    IfComments, build_if_header, collect_if_comments, format_else_with_comments,
    lower_comments_before_else, lower_if_with_comments,
};
use crate::lower::lower_node;

fn is_braced_body(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "{" || child.kind() == "block" {
            return true;
        }
    }
    false
}

pub(crate) fn extract_single_if_child<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if node.kind() == "if_expression" {
        return Some(node);
    }
    node.children(&mut node.walk())
        .find(|child| child.kind() == "if_expression")
}

pub(crate) fn is_control_statement(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "do_while_statement" | "while_statement" | "for_statement"
        ) {
            return true;
        }
    }
    matches!(
        node.kind(),
        "do_while_statement" | "while_statement" | "for_statement"
    )
}

fn is_inside_when_entry(node: &Node<'_>) -> bool {
    let mut curr = node.parent();
    while let Some(p) = curr {
        if p.kind() == "when_entry" {
            return true;
        }
        if matches!(
            p.kind(),
            "statements" | "block" | "class_body" | "function_body"
        ) {
            return false;
        }
        curr = p.parent();
    }
    false
}

fn lower_braced_if<'a>(
    mut parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    comments: &IfComments<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    parts.push(Doc::space());
    if let Some(cons) = consequence {
        parts.push(lower_node(cons, ctx));
    }
    if alternative.is_none() && comments.between_cons_and_else.is_empty() {
        return Doc::Concat(parts);
    }
    if !comments.between_cons_and_else.is_empty() {
        for c in &comments.between_cons_and_else {
            parts.push(Doc::HardLine);
            parts.push(c.clone());
        }
        parts.push(Doc::HardLine);
        parts.push(Doc::text("else"));
        if !comments.between_else_and_alt.is_empty() {
            parts.push(format_else_with_comments(
                &comments.between_else_and_alt,
                alternative,
                ctx,
            ));
        } else if let Some(if_node) = alternative.and_then(extract_single_if_child) {
            parts.push(Doc::space());
            parts.push(lower_node(if_node, ctx));
        } else if let Some(alt) = alternative {
            parts.push(Doc::space());
            parts.push(lower_node(alt, ctx));
        }
    } else if !comments.between_else_and_alt.is_empty() {
        parts.push(Doc::text(" else"));
        parts.push(format_else_with_comments(
            &comments.between_else_and_alt,
            alternative,
            ctx,
        ));
    } else if let Some(if_node) = alternative.and_then(extract_single_if_child) {
        parts.push(Doc::text(" else "));
        parts.push(lower_node(if_node, ctx));
    } else if let Some(alt) = alternative {
        parts.push(Doc::text(" else "));
        parts.push(lower_node(alt, ctx));
    }
    Doc::Concat(parts)
}

fn lower_unbraced_else<'a>(
    alternative: Option<Node<'a>>,
    has_else_semi: bool,
    in_when: bool,
    ctx: &LowerContext<'a, '_>,
) -> Vec<Doc<'a>> {
    let mut parts = Vec::new();
    let alt_is_if = alternative.and_then(extract_single_if_child);
    if in_when {
        if let Some(if_node) = alt_is_if {
            parts.push(Doc::space());
            parts.push(Doc::text("else "));
            parts.push(lower_node(if_node, ctx));
        } else if let Some(alt) = alternative {
            parts.push(Doc::space());
            parts.push(Doc::text("else "));
            parts.push(lower_node(alt, ctx));
        }
    } else if let Some(if_node) = alt_is_if {
        parts.push(Doc::Line);
        parts.push(Doc::text("else "));
        parts.push(lower_node(if_node, ctx));
    } else if let Some(alt) = alternative {
        if is_control_statement(alt) {
            parts.push(Doc::HardLine);
            parts.push(Doc::text("else"));
            parts.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_node(alt, ctx),
            ])));
        } else {
            parts.push(Doc::Line);
            parts.push(Doc::Group(Box::new(Doc::Concat(vec![
                Doc::text("else"),
                Doc::continuation_indent(Doc::Concat(vec![Doc::Line, lower_node(alt, ctx)])),
            ]))));
        }
    } else if has_else_semi {
        parts.push(Doc::Line);
        parts.push(Doc::text("else ;"));
    }
    parts
}

fn lower_unbraced_consequence<'a>(
    mut if_branch_parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    has_else_semi: bool,
    in_when: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if let Some(cons) = consequence {
        if in_when {
            if_branch_parts.push(Doc::space());
            if_branch_parts.push(lower_node(cons, ctx));
        } else if is_control_statement(cons) {
            if_branch_parts.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_node(cons, ctx),
            ])));
        } else {
            if_branch_parts.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::Line,
                lower_node(cons, ctx),
            ])));
        }
    } else if alternative.is_some() || has_else_semi {
        if_branch_parts.push(Doc::space());
    } else {
        if_branch_parts.push(Doc::space());
        if_branch_parts.push(Doc::text(";"));
    }
    if in_when {
        Doc::Concat(if_branch_parts)
    } else {
        Doc::Group(Box::new(Doc::Concat(if_branch_parts)))
    }
}

fn lower_unbraced_if<'a>(
    node: Node<'a>,
    if_header_parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    comments: &IfComments<'a>,
    has_else_semi: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let in_when = is_inside_when_entry(&node);
    let if_branch_doc = lower_unbraced_consequence(
        if_header_parts,
        consequence,
        alternative,
        has_else_semi,
        in_when,
        ctx,
    );

    let mut all_parts = vec![if_branch_doc];
    if !comments.between_cons_and_else.is_empty() {
        all_parts.extend(lower_comments_before_else(
            &comments.between_cons_and_else,
            &comments.between_else_and_alt,
            alternative,
            ctx,
        ));
    } else if !comments.between_else_and_alt.is_empty() {
        all_parts.push(Doc::HardLine);
        all_parts.push(Doc::text("else"));
        all_parts.push(format_else_with_comments(
            &comments.between_else_and_alt,
            alternative,
            ctx,
        ));
    } else {
        all_parts.extend(lower_unbraced_else(
            alternative,
            has_else_semi,
            in_when,
            ctx,
        ));
    }
    if in_when {
        Doc::Concat(all_parts)
    } else {
        Doc::Group(Box::new(Doc::Concat(all_parts)))
    }
}

pub fn lower_if_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let cond = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let alternative = node.child_by_field_name("alternative");

    let cond_doc = cond.map(|c| lower_node(c, ctx)).unwrap_or_else(Doc::empty);
    let comments = collect_if_comments(&node, cond, consequence, alternative, ctx);

    let if_header = build_if_header(cond_doc, &comments, ctx);
    let parts = vec![if_header];
    let cons_is_braced = consequence.is_some_and(is_braced_body);

    let has_empty_semi =
        consequence.is_none() && node.children(&mut node.walk()).any(|c| c.kind() == ";");
    let has_else_semi =
        alternative.is_none() && node.children(&mut node.walk()).any(|c| c.kind() == "else");

    let res = if !comments.before_cons.is_empty() {
        lower_if_with_comments(
            parts,
            consequence,
            alternative,
            &comments,
            has_empty_semi,
            ctx,
        )
    } else if cons_is_braced {
        lower_braced_if(parts, consequence, alternative, &comments, ctx)
    } else {
        lower_unbraced_if(
            node,
            parts,
            consequence,
            alternative,
            &comments,
            has_else_semi,
            ctx,
        )
    };
    if is_receiver_of_broken_navigation(&node, ctx) {
        Doc::continuation_indent(res)
    } else {
        res
    }
}

fn is_receiver_of_broken_navigation(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() == "navigation_expression"
        && parent.child(0).is_some_and(|c| c.id() == node.id())
    {
        return ctx.node_text(&parent).contains('\n');
    }
    false
}
