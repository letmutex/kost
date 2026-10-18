use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
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

fn extract_single_if_child<'a>(node: Node<'a>) -> Option<Node<'a>> {
    if node.kind() == "if_expression" {
        return Some(node);
    }
    node.children(&mut node.walk())
        .find(|child| child.kind() == "if_expression")
}

fn is_control_statement(node: Node<'_>) -> bool {
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

fn lower_if_with_comments<'a>(
    parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    comments: Vec<Doc<'a>>,
    has_empty_semi: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut out = parts;
    if let Some(cons) = consequence {
        let mut cons_parts = Vec::new();
        for c in comments {
            cons_parts.push(c);
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
        for c in comments {
            out.push(Doc::HardLine);
            out.push(c);
        }
        out.push(Doc::HardLine);
        out.push(Doc::space());
        out.push(Doc::text(";"));
    }
    if let Some(alt) = alternative {
        out.push(Doc::HardLine);
        out.push(Doc::text("else"));
        out.push(Doc::continuation_indent(Doc::Concat(vec![
            Doc::HardLine,
            lower_node(alt, ctx),
        ])));
    }
    Doc::Concat(out)
}

fn lower_braced_if<'a>(
    mut parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    parts.push(Doc::space());
    if let Some(cons) = consequence {
        parts.push(lower_node(cons, ctx));
    }
    let alt_is_if = alternative.and_then(extract_single_if_child);
    let alt_is_braced = alternative.is_some_and(is_braced_body);
    if let Some(if_node) = alt_is_if {
        parts.push(Doc::text(" else "));
        parts.push(lower_node(if_node, ctx));
    } else if alt_is_braced {
        parts.push(Doc::text(" else "));
        if let Some(alt) = alternative {
            parts.push(lower_node(alt, ctx));
        }
    } else if let Some(alt) = alternative {
        parts.push(Doc::text(" else "));
        parts.push(lower_node(alt, ctx));
    }
    Doc::Concat(parts)
}

fn lower_unbraced_if<'a>(
    if_header_parts: Vec<Doc<'a>>,
    consequence: Option<Node<'a>>,
    alternative: Option<Node<'a>>,
    has_empty_semi: bool,
    has_else_semi: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut if_branch_parts = if_header_parts;
    if let Some(cons) = consequence {
        if is_control_statement(cons) {
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
    } else if has_empty_semi {
        if_branch_parts.push(Doc::space());
        if_branch_parts.push(Doc::text(";"));
    }
    let if_branch_doc = Doc::Group(Box::new(Doc::Concat(if_branch_parts)));

    let mut all_parts = vec![if_branch_doc];
    let alt_is_if = alternative.and_then(extract_single_if_child);
    if let Some(if_node) = alt_is_if {
        all_parts.push(Doc::Line);
        all_parts.push(Doc::text("else "));
        all_parts.push(lower_node(if_node, ctx));
    } else if let Some(alt) = alternative {
        if is_control_statement(alt) {
            all_parts.push(Doc::HardLine);
            all_parts.push(Doc::text("else"));
            all_parts.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_node(alt, ctx),
            ])));
        } else {
            all_parts.push(Doc::Line);
            all_parts.push(Doc::Group(Box::new(Doc::Concat(vec![
                Doc::text("else"),
                Doc::continuation_indent(Doc::Concat(vec![Doc::Line, lower_node(alt, ctx)])),
            ]))));
        }
    } else if has_else_semi {
        all_parts.push(Doc::Line);
        all_parts.push(Doc::text("else ;"));
    }
    Doc::Group(Box::new(Doc::Concat(all_parts)))
}

pub fn lower_if_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let cond = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let alternative = node.child_by_field_name("alternative");

    let cond_doc = cond.map(|c| lower_node(c, ctx)).unwrap_or_else(Doc::empty);

    let mut comments_before_cons = Vec::new();
    let mut after_rparen = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ")" {
            after_rparen = true;
        } else if Some(child) == consequence {
            break;
        } else if after_rparen && matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            comments_before_cons.push(lower_node(child, ctx));
        }
    }

    let if_header = if ctx.options.trailing_comma_strategy != crate::TrailingCommaStrategy::None {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text("if ("),
            Doc::indent(Doc::Concat(vec![Doc::SoftLine, cond_doc])),
            Doc::SoftLine,
            Doc::text(")"),
        ])))
    } else {
        Doc::Concat(vec![Doc::text("if ("), cond_doc, Doc::text(")")])
    };
    let parts = vec![if_header];
    let cons_is_braced = consequence.is_some_and(is_braced_body);

    let has_empty_semi =
        consequence.is_none() && node.children(&mut node.walk()).any(|c| c.kind() == ";");
    let has_else_semi =
        alternative.is_none() && node.children(&mut node.walk()).any(|c| c.kind() == "else");

    let res = if !comments_before_cons.is_empty() {
        lower_if_with_comments(
            parts,
            consequence,
            alternative,
            comments_before_cons,
            has_empty_semi,
            ctx,
        )
    } else if cons_is_braced {
        lower_braced_if(parts, consequence, alternative, ctx)
    } else {
        lower_unbraced_if(
            parts,
            consequence,
            alternative,
            has_empty_semi,
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

pub fn lower_jump_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut prev_is_at = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let is_at = ctx.node_text(&child).ends_with('@');
        if !docs.is_empty() && !prev_is_at {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
        prev_is_at = is_at;
    }
    Doc::Concat(docs)
}

pub fn lower_control_structure_body<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut has_braces = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "{" {
            has_braces = true;
            break;
        }
    }

    if has_braces {
        crate::lower::expr::lower_block(node, ctx)
    } else {
        let mut docs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            docs.push(lower_node(child, ctx));
        }
        Doc::Concat(docs)
    }
}

pub fn lower_type_or_range_test<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut op_text = String::new();
    let mut target = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "is" || kind == "in" {
            op_text.push_str(ctx.node_text(&child));
        } else if kind == "!" {
            op_text.push('!');
        } else {
            target = Some(lower_node(child, ctx));
        }
    }
    let target_doc = target.unwrap_or_else(Doc::empty);
    Doc::Concat(vec![Doc::smol(op_text), Doc::space(), target_doc])
}
