use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

fn has_property_accessor(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "getter" | "setter") {
            return true;
        }
    }
    false
}

fn requires_blank_line_separation(prev: &Node<'_>, next: &Node<'_>) -> bool {
    if matches!(
        next.kind(),
        "getter" | "setter" | "comment" | "line_comment" | "multiline_comment"
    ) {
        return false;
    }
    if prev.kind() == ";" {
        return matches!(
            next.kind(),
            "function_declaration"
                | "class_declaration"
                | "object_declaration"
                | "property_declaration"
                | "anonymous_initializer"
                | "secondary_constructor"
                | "companion_object"
        );
    }
    let is_heavy = |k: &str| {
        matches!(
            k,
            "function_declaration"
                | "class_declaration"
                | "object_declaration"
                | "anonymous_initializer"
                | "secondary_constructor"
                | "companion_object"
                | "getter"
                | "setter"
        )
    };
    if is_heavy(prev.kind()) || is_heavy(next.kind()) {
        return true;
    }
    if prev.kind() == "property_declaration" && has_property_accessor(prev) {
        return true;
    }
    false
}

fn handle_member_delimiter<'a>(
    child: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    prev_member: Option<Node<'a>>,
    member_docs: &mut Vec<Doc<'a>>,
) -> bool {
    let kind = child.kind();
    if kind != "," && kind != ";" {
        return false;
    }
    if kind == ";" && ctx.is_redundant_semicolon(&child) {
        if prev_member.is_some_and(|p| p.kind() == "enum_entry")
            && ctx.options.trailing_comma_strategy != crate::options::TrailingCommaStrategy::None
            && !member_docs
                .last()
                .is_some_and(|d| matches!(d, Doc::Text(",")))
        {
            member_docs.push(Doc::text(","));
        }
        return true;
    }
    if kind == ";"
        && let Some(prev) = prev_member
        && ctx.has_newline_between(&prev, &child)
    {
        member_docs.push(Doc::HardLine);
    }
    member_docs.push(Doc::text(ctx.node_text(&child)));
    true
}

fn lower_class_member<'a>(
    child: Node<'a>,
    last_enum_entry: Option<Node<'a>>,
    has_terminating_semicolon: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut entry_doc = lower_node(child, ctx);
    if child.kind() == "enum_entry"
        && Some(child) == last_enum_entry
        && !has_terminating_semicolon
        && ctx.options.trailing_comma_strategy != crate::options::TrailingCommaStrategy::None
        && !child.next_sibling().is_some_and(|s| s.kind() == ",")
    {
        entry_doc = Doc::Concat(vec![entry_doc, Doc::text(",")]);
    }
    entry_doc
}

fn process_class_member<'a>(
    child: Node<'a>,
    last_enum_entry: Option<Node<'a>>,
    has_terminating_semicolon: bool,
    prev_member: &mut Option<Node<'a>>,
    member_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let kind = child.kind();
    if kind == ","
        && child.next_sibling().is_some_and(|next| {
            next.kind() == ";"
                && !ctx.is_redundant_semicolon(&next)
                && next.start_position().row == child.start_position().row
        })
    {
        return;
    }

    if handle_member_delimiter(child, ctx, *prev_member, member_docs) {
        if !(child.kind() == ";" && ctx.is_redundant_semicolon(&child)) {
            *prev_member = Some(child);
        }
        return;
    }

    if matches!(kind, "getter" | "setter" | "backing_field") {
        member_docs.push(Doc::indent(Doc::Concat(vec![
            Doc::HardLine,
            lower_node(child, ctx),
        ])));
    } else {
        let entry_doc = lower_class_member(child, last_enum_entry, has_terminating_semicolon, ctx);
        if let Some(prev) = *prev_member {
            separate_class_members(&prev, &child, ctx, member_docs);
        }
        member_docs.push(entry_doc);
    }
    *prev_member = Some(child);
}

fn class_body_first_line<'a>(
    open_brace: Option<Node<'a>>,
    first_token_after_brace: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if let (Some(ob), Some(fm)) = (open_brace, first_token_after_brace) {
        if ctx.has_blank_line_between(&ob, &fm) {
            Doc::EmptyLine
        } else if !ctx.has_newline_between(&ob, &fm)
            && matches!(fm.kind(), "comment" | "line_comment" | "multiline_comment")
        {
            Doc::space()
        } else {
            Doc::HardLine
        }
    } else {
        Doc::HardLine
    }
}

fn class_body_last_line<'a>(
    prev_member: Option<Node<'a>>,
    close_brace: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if let (Some(last), Some(cb)) = (prev_member, close_brace) {
        if ctx.has_blank_line_between(&last, &cb) {
            Doc::EmptyLine
        } else {
            Doc::HardLine
        }
    } else {
        Doc::HardLine
    }
}

fn separate_class_members<'a>(
    prev: &Node<'a>,
    child: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
    member_docs: &mut Vec<Doc<'a>>,
) {
    if prev.kind() == "annotation" && child.kind() == "parenthesized_expression" {
        return;
    }
    if ctx.has_blank_line_between(prev, child) || requires_blank_line_separation(prev, child) {
        member_docs.push(Doc::EmptyLine);
    } else if matches!(prev.kind(), "comment" | "multiline_comment")
        && ctx.node_text(prev).starts_with("/**")
    {
        if !member_docs.last().is_some_and(|d| d.ends_with_newline()) {
            member_docs.push(Doc::HardLine);
        }
    } else if child.kind() == "enum_entry" || ctx.has_newline_between(prev, child) {
        if !member_docs.last().is_some_and(|d| d.ends_with_newline()) {
            member_docs.push(Doc::HardLine);
        }
    } else {
        member_docs.push(Doc::space());
    }
}

pub fn lower_class_body<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut member_docs = Vec::new();
    let mut cursor = node.walk();
    let mut prev_member: Option<Node<'a>> = None;
    let mut open_brace: Option<Node<'a>> = None;
    let mut first_token_after_brace: Option<Node<'a>> = None;

    let has_terminating_semicolon = node.children(&mut node.walk()).any(|c| c.kind() == ";");
    let last_enum_entry = node
        .children(&mut node.walk())
        .filter(|c| c.kind() == "enum_entry")
        .last();

    let mut close_brace: Option<Node<'a>> = None;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "{" => open_brace = Some(child),
            "}" => close_brace = Some(child),
            _ => {
                if first_token_after_brace.is_none() {
                    first_token_after_brace = Some(child);
                }
                process_class_member(
                    child,
                    last_enum_entry,
                    has_terminating_semicolon,
                    &mut prev_member,
                    &mut member_docs,
                    ctx,
                );
            }
        }
    }

    if member_docs.is_empty() {
        return Doc::text("{}");
    }

    let first_line = class_body_first_line(open_brace, first_token_after_brace, ctx);
    let last_line = class_body_last_line(prev_member, close_brace, ctx);

    Doc::Concat(vec![
        Doc::text("{"),
        Doc::indent(Doc::Concat(vec![first_line, Doc::Concat(member_docs)])),
        last_line,
        Doc::text("}"),
    ])
}

pub fn lower_companion_object<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut has_body = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body" {
            has_body = true;
        }
        if !docs.is_empty() && child.kind() != ";" {
            docs.push(Doc::space());
        }
        docs.push(lower_node(child, ctx));
    }
    if !has_body && node.next_named_sibling().is_some() {
        let after = ctx.source.get(node.end_byte()..).unwrap_or("");
        if after.trim_start().starts_with(';') {
            docs.push(Doc::text(";"));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_anonymous_initializer<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut inner_docs = Vec::new();
    let mut cursor = node.walk();
    let mut inside_braces = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" {
            inside_braces = true;
            continue;
        }
        if kind == "}" {
            break;
        }
        if inside_braces {
            inner_docs.push(lower_node(child, ctx));
        }
    }

    if inner_docs.is_empty() {
        Doc::text("init {}")
    } else {
        Doc::Concat(vec![
            Doc::text("init {"),
            Doc::indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(inner_docs)])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    }
}
