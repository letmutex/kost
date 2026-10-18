use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_when_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut subject_doc = None;
    let mut entries = Vec::new();
    let mut prev_child: Option<Node<'a>> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "when_subject" {
            subject_doc = Some(lower_when_subject(child, ctx));
        } else if kind == "when_entry" {
            let has_blank =
                prev_child.is_some_and(|prev| ctx.has_blank_line_between(&prev, &child));
            entries.push((lower_when_entry(child, ctx), has_blank));
            prev_child = Some(child);
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            let has_blank =
                prev_child.is_some_and(|prev| ctx.has_blank_line_between(&prev, &child));
            entries.push((lower_node(child, ctx), has_blank));
            prev_child = Some(child);
        }
    }

    let mut header = vec![Doc::text("when")];
    if let Some(subject) = subject_doc {
        header.push(Doc::space());
        header.push(subject);
    }
    header.push(Doc::space());
    header.push(Doc::text("{"));

    if entries.is_empty() {
        header.push(Doc::text("}"));
        return Doc::Concat(header);
    }

    let mut inner = Vec::new();
    for (idx, (entry_doc, has_blank)) in entries.into_iter().enumerate() {
        if idx == 0 {
            inner.push(Doc::HardLine);
        } else if has_blank {
            inner.push(Doc::EmptyLine);
        } else {
            inner.push(Doc::HardLine);
        }
        inner.push(entry_doc);
    }

    Doc::Concat(vec![
        Doc::Concat(header),
        Doc::indent(Doc::Concat(inner)),
        Doc::HardLine,
        Doc::text("}"),
    ])
}

pub fn lower_when_subject<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut inner = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "(" | ")" => {}
            "val" => inner.push(Doc::text("val ")),
            "var" => inner.push(Doc::text("var ")),
            "=" => inner.push(Doc::text(" = ")),
            _ => inner.push(lower_node(child, ctx)),
        }
    }
    if ctx.options.trailing_comma_strategy != crate::TrailingCommaStrategy::None {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text("("),
            Doc::indent(Doc::Concat(vec![Doc::SoftLine, Doc::Concat(inner)])),
            Doc::SoftLine,
            Doc::text(")"),
        ])))
    } else {
        Doc::Concat(vec![Doc::text("("), Doc::Concat(inner), Doc::text(")")])
    }
}

fn is_braced_body(node: Node<'_>) -> bool {
    if node.kind() == "control_structure_body" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(
                child.kind(),
                "{" | "block" | "lambda_literal" | "annotated_lambda"
            ) || is_braced_body(child)
            {
                return true;
            }
        }
        return false;
    }
    matches!(
        node.kind(),
        "block" | "{" | "lambda_literal" | "annotated_lambda"
    )
}

fn lower_guard_condition<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut parts = vec![Doc::text("if ")];
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "if" {
            parts.push(lower_node(child, ctx));
        }
    }
    Doc::Concat(parts)
}

fn build_when_entry_head<'a>(
    conditions: Vec<Node<'a>>,
    guard: Option<Node<'a>>,
    has_trailing_comma: bool,
    arrow_has_newline: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut head_parts = Vec::new();
    if conditions.is_empty() {
        head_parts.push(Doc::text("else"));
    } else {
        let mut prev_item: Option<Node<'a>> = None;
        for item in conditions {
            let is_comment = matches!(
                item.kind(),
                "comment" | "line_comment" | "multiline_comment"
            );
            if let Some(prev) = prev_item {
                let prev_is_comment = matches!(
                    prev.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                );
                let should_comma = is_comment || !prev_is_comment;
                if should_comma {
                    if ctx.has_newline_between(&prev, &item) {
                        head_parts.push(Doc::text(","));
                        head_parts.push(Doc::HardLine);
                    } else {
                        head_parts.push(Doc::text(", "));
                    }
                }
            }
            head_parts.push(lower_node(item, ctx));
            prev_item = Some(item);
        }
        if has_trailing_comma
            && (arrow_has_newline
                || ctx.options.trailing_comma_strategy == crate::TrailingCommaStrategy::OnlyAdd)
        {
            head_parts.push(Doc::text(","));
        }
    }
    if let Some(g) = guard {
        head_parts.push(Doc::space());
        head_parts.push(lower_guard_condition(g, ctx));
    }
    if arrow_has_newline {
        head_parts.push(Doc::HardLine);
        head_parts.push(Doc::text("->"));
    } else {
        head_parts.push(Doc::text(" ->"));
    }
    Doc::Concat(head_parts)
}

fn format_when_entry_body<'a>(
    head: Doc<'a>,
    body_node: Option<Node<'a>>,
    comments_before_body: Vec<Node<'a>>,
    comments_after_body: Vec<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let Some(body) = body_node else {
        return head;
    };
    let body_doc = lower_node(body, ctx);
    let main_doc = if !comments_before_body.is_empty() {
        let mut inner_comments = Vec::new();
        for comment in comments_before_body {
            inner_comments.push(Doc::HardLine);
            inner_comments.push(lower_node(comment, ctx));
        }
        inner_comments.push(Doc::HardLine);
        inner_comments.push(body_doc);
        Doc::Concat(vec![
            head,
            Doc::continuation_indent(Doc::Concat(inner_comments)),
        ])
    } else if is_braced_body(body) {
        Doc::Concat(vec![head, Doc::space(), body_doc])
    } else {
        Doc::Concat(vec![
            head,
            Doc::Group(Box::new(Doc::continuation_indent(Doc::Concat(vec![
                Doc::Line,
                body_doc,
            ])))),
        ])
    };

    if comments_after_body.is_empty() {
        main_doc
    } else {
        let mut parts = vec![main_doc];
        let mut prev = body;
        for comment in comments_after_body {
            if prev.end_position().row == comment.start_position().row {
                parts.push(Doc::space());
            } else {
                parts.push(Doc::HardLine);
            }
            parts.push(lower_node(comment, ctx));
            prev = comment;
        }
        Doc::Concat(parts)
    }
}

pub fn lower_when_entry<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut conditions = Vec::new();
    let mut guard_node = None;
    let mut comments_before_body = Vec::new();
    let mut comments_after_body = Vec::new();
    let mut body_node = None;
    let mut after_arrow = false;
    let mut has_trailing_comma = false;
    let mut arrow_node = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "->" {
            after_arrow = true;
            arrow_node = Some(child);
            continue;
        }

        if !after_arrow {
            if kind == "," {
                has_trailing_comma = true;
            } else if kind == "guard_condition" {
                guard_node = Some(child);
            } else if kind == "when_condition"
                || kind == "else"
                || matches!(kind, "comment" | "line_comment" | "multiline_comment")
            {
                has_trailing_comma = false;
                conditions.push(child);
            }
        } else {
            let is_comment = matches!(kind, "comment" | "line_comment" | "multiline_comment");
            if is_comment && body_node.is_none() {
                comments_before_body.push(child);
            } else if is_comment {
                comments_after_body.push(child);
            } else if body_node.is_none() {
                body_node = Some(child);
            }
        }
    }

    let last_before_arrow = guard_node.or_else(|| conditions.last().copied());
    let arrow_has_newline = match (last_before_arrow, arrow_node) {
        (Some(l), Some(a)) => ctx.has_newline_between(&l, &a),
        _ => false,
    };

    let head = build_when_entry_head(
        conditions,
        guard_node,
        has_trailing_comma,
        arrow_has_newline,
        ctx,
    );
    format_when_entry_body(
        head,
        body_node,
        comments_before_body,
        comments_after_body,
        ctx,
    )
}
