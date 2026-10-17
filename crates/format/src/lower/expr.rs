use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_block<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut stmt_docs = Vec::new();
    let mut prev_child: Option<Node<'a>> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" || kind == "}" {
            continue;
        }
        if let Some(prev) = prev_child {
            if ctx.has_blank_line_between(&prev, &child) {
                stmt_docs.push(Doc::EmptyLine);
            } else if !ctx.has_newline_between(&prev, &child) {
                stmt_docs.push(Doc::space());
            } else if !stmt_docs.last().is_some_and(|d| d.ends_with_newline()) {
                stmt_docs.push(Doc::HardLine);
            }
        }
        stmt_docs.push(lower_node(child, ctx));
        prev_child = Some(child);
    }

    if stmt_docs.is_empty() {
        return Doc::text("{}");
    }

    Doc::Concat(vec![
        Doc::text("{"),
        Doc::indent(Doc::Concat(vec![Doc::HardLine, Doc::Concat(stmt_docs)])),
        Doc::HardLine,
        Doc::text("}"),
    ])
}

fn statement_starts_with_call_ambiguity(node: Node<'_>) -> bool {
    let mut curr = node;
    loop {
        match curr.kind() {
            "lambda_literal" | "annotated_lambda" | "parenthesized_expression" => return true,
            "navigation_expression"
            | "postfix_expression"
            | "as_expression"
            | "check_expression"
            | "call_expression" => {
                if let Some(first) = curr.child(0) {
                    curr = first;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

fn has_following_call_ambiguity(node: Node<'_>) -> bool {
    let mut next = node.next_sibling();
    while let Some(n) = next {
        if !matches!(
            n.kind(),
            "comment" | "line_comment" | "multiline_comment" | ";"
        ) {
            return statement_starts_with_call_ambiguity(n);
        }
        next = n.next_sibling();
    }
    false
}

fn separate_statements<'a>(
    prev: Node<'a>,
    child: Node<'a>,
    stmt_docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let is_anno_arg = prev.kind() == "annotation"
        && !ctx.has_newline_between(&prev, &child)
        && (child.kind() == "parenthesized_expression"
            || (child.kind() == "infix_expression"
                && child
                    .child(0)
                    .is_some_and(|c| c.kind() == "parenthesized_expression")));

    if is_anno_arg {
        return;
    }

    let span = ctx
        .source
        .get(prev.end_byte()..child.start_byte())
        .unwrap_or("");
    let child_is_comment = matches!(
        child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let prev_is_comment = matches!(
        prev.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let child_is_kdoc = child_is_comment && ctx.node_text(&child).starts_with("/**");

    let needs_semi = span.contains(';')
        && (statement_starts_with_call_ambiguity(child)
            || (child_is_comment && has_following_call_ambiguity(child)));

    if needs_semi {
        if prev_is_comment
            && !stmt_docs
                .last()
                .is_some_and(|d: &Doc<'_>| d.ends_with_newline())
        {
            stmt_docs.push(Doc::HardLine);
        }
        stmt_docs.push(Doc::text(";"));
    }

    let prev_is_kdoc = prev_is_comment && ctx.node_text(&prev).starts_with("/**");
    let same_line_comment = ((child_is_comment && !child_is_kdoc)
        || (prev.kind() == "annotation" && !child_is_kdoc)
        || (prev_is_comment && child.kind() == "annotation" && !prev_is_kdoc))
        && !needs_semi
        && !ctx.has_newline_between(&prev, &child);
    if prev.kind() == "label" || same_line_comment {
        stmt_docs.push(Doc::space());
    } else if !needs_semi && ctx.has_blank_line_between(&prev, &child) {
        stmt_docs.push(Doc::EmptyLine);
    } else if !stmt_docs.last().is_some_and(|d| d.ends_with_newline()) {
        stmt_docs.push(Doc::HardLine);
    }
}

pub fn lower_statements<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut stmt_docs = Vec::new();
    let mut cursor = node.walk();
    let mut prev_stmt: Option<Node<'a>> = None;

    for child in node.children(&mut cursor) {
        if child.kind() == ";" {
            continue;
        }
        if let Some(prev) = prev_stmt {
            separate_statements(prev, child, &mut stmt_docs, ctx);
        }

        let is_infix_after_anno = prev_stmt.is_some_and(|p| {
            p.kind() == "annotation"
                && !ctx.has_newline_between(&p, &child)
                && child.kind() == "infix_expression"
                && child
                    .child(0)
                    .is_some_and(|c| c.kind() == "parenthesized_expression")
        });

        let doc = if is_infix_after_anno {
            lower_annotated_statement_infix(child, ctx)
        } else {
            lower_node(child, ctx)
        };
        stmt_docs.push(doc);
        prev_stmt = Some(child);
    }

    Doc::Concat(stmt_docs)
}

fn collect_binary_elements<'a>(node: Node<'a>, parent_kind: &str, elements: &mut Vec<Node<'a>>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == parent_kind {
            collect_binary_elements(child, parent_kind, elements);
        } else {
            elements.push(child);
        }
    }
}

fn is_binary_operator(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "&&"
            | "||"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "<"
            | "<="
            | ">"
            | ">="
            | "?:"
            | "as"
            | "as?"
            | "is"
            | "!is"
            | ".."
            | "..<"
            | "custom_operation_symbol"
            | "in"
            | "!in"
    )
}

#[derive(Default)]
struct BinaryLowerState<'a> {
    is_after_comment: bool,
    prev_was_operator: bool,
    pending_elvis: bool,
    elvis_newline: bool,
    prev_child: Option<Node<'a>>,
}

fn push_binary_child_doc<'a>(
    docs: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    state: &mut BinaryLowerState<'a>,
    ctx: &LowerContext<'a, '_>,
) {
    let kind = child.kind();
    let is_comment = matches!(kind, "comment" | "line_comment" | "multiline_comment");
    let is_operator = is_binary_operator(&child);

    if docs.is_empty() {
        docs.push(lower_node(child, ctx));
    } else if is_comment {
        if state
            .prev_child
            .is_some_and(|p| !ctx.has_newline_between(&p, &child))
        {
            docs.push(Doc::space());
            docs.push(lower_node(child, ctx));
            state.is_after_comment = false;
        } else {
            docs.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::Line,
                lower_node(child, ctx),
            ])));
            state.is_after_comment = true;
        }
        state.prev_was_operator = false;
    } else if is_operator && kind == "?:" {
        state.pending_elvis = true;
        state.elvis_newline = state
            .prev_child
            .is_some_and(|p| ctx.has_newline_between(&p, &child));
    } else if state.pending_elvis {
        let line_doc = if state.elvis_newline {
            Doc::HardLine
        } else {
            Doc::Line
        };
        docs.push(Doc::continuation_indent(Doc::Concat(vec![
            line_doc,
            Doc::text("?: "),
            lower_node(child, ctx),
        ])));
        state.pending_elvis = false;
        state.prev_was_operator = false;
    } else if is_operator && state.is_after_comment {
        let op_doc = if docs.last().is_some_and(|d| d.ends_with_newline()) {
            Doc::text(ctx.node_text(&child))
        } else {
            Doc::Concat(vec![Doc::Line, Doc::text(ctx.node_text(&child))])
        };
        docs.push(Doc::continuation_indent(op_doc));
        state.prev_was_operator = true;
    } else if is_operator {
        docs.push(Doc::space());
        docs.push(Doc::text(ctx.node_text(&child)));
        state.prev_was_operator = true;
    } else if state.prev_was_operator {
        if state.is_after_comment {
            docs.push(Doc::space());
            docs.push(lower_node(child, ctx));
            state.is_after_comment = false;
        } else {
            docs.push(Doc::continuation_indent(Doc::Concat(vec![
                Doc::Line,
                lower_node(child, ctx),
            ])));
        }
        state.prev_was_operator = false;
    } else {
        docs.push(Doc::space());
        docs.push(lower_node(child, ctx));
    }
    state.prev_child = Some(child);
}

pub fn lower_binary_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut elements = Vec::new();
    collect_binary_elements(node, node.kind(), &mut elements);

    if elements.is_empty() {
        return Doc::empty();
    }

    let mut docs = Vec::new();
    let mut state = BinaryLowerState::default();

    for child in elements {
        push_binary_child_doc(&mut docs, child, &mut state, ctx);
    }

    Doc::Group(Box::new(Doc::Concat(docs)))
}

fn lower_annotated_statement_infix<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut children = node.children(&mut cursor);
    if let Some(arg) = children.next() {
        docs.push(lower_node(arg, ctx));
        docs.push(Doc::space());
    }
    let mut prev_c: Option<Node<'a>> = None;
    for c in children {
        if prev_c.is_some_and(|p| p.end_byte() < c.start_byte()) {
            docs.push(Doc::space());
        }
        docs.push(lower_node(c, ctx));
        prev_c = Some(c);
    }
    Doc::Concat(docs)
}
