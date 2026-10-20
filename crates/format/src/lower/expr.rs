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

fn statement_annotation_fits(
    prev: &Node<'_>,
    child: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let indent = ctx.compute_target_indent(prev);
    let anno_len = ctx.node_text(prev).len();
    let stmt_len = if child.kind() == "assignment" {
        let lhs = child.child(0).map(|c| ctx.node_text(&c).len()).unwrap_or(0);
        lhs + 3
    } else {
        crate::lower::expression_body::collapsed_text_len(
            ctx.node_text(child).lines().next().unwrap_or(""),
        )
    };
    let comment_len = if let Some(next) = child.next_sibling()
        && matches!(
            next.kind(),
            "comment" | "line_comment" | "multiline_comment"
        )
        && !ctx.has_newline_between(child, &next)
    {
        1 + ctx.node_text(&next).len()
    } else {
        0
    };
    indent + anno_len + 1 + stmt_len + comment_len <= ctx.options.max_width
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
    let prev_is_anno = prev.kind() == "annotation" && !child_is_kdoc;
    let anno_fits = if prev_is_anno && !ctx.has_newline_between(&prev, &child) {
        statement_annotation_fits(&prev, &child, ctx)
    } else {
        true
    };

    let same_line_comment = ((child_is_comment && !child_is_kdoc)
        || (prev_is_anno && anno_fits)
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

fn find_statement_expr<'a>(node: Node<'a>) -> Node<'a> {
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        let k = ch.kind();
        if k == "=" || k == "by" || k == "property_delegate" {
            let mut next = ch.next_named_sibling();
            while let Some(n) = next {
                if !matches!(n.kind(), "comment" | "line_comment" | "multiline_comment") {
                    return n;
                }
                next = n.next_named_sibling();
            }
        }
    }
    if node.kind() == "jump_expression" {
        let mut cursor = node.walk();
        for ch in node.children(&mut cursor) {
            if !matches!(
                ch.kind(),
                "return"
                    | "continue"
                    | "break"
                    | "throw"
                    | "label"
                    | "comment"
                    | "line_comment"
                    | "multiline_comment"
            ) {
                return ch;
            }
        }
    }
    node
}

fn is_completed_callee(callee: Node<'_>) -> bool {
    if matches!(
        callee.kind(),
        "lambda_literal" | "as_expression" | "check_expression"
    ) {
        return true;
    }
    if callee.kind() == "call_expression" {
        let mut cursor = callee.walk();
        for ch in callee.children(&mut cursor) {
            if ch.kind() == "call_suffix" {
                return true;
            }
        }
    }
    false
}

fn find_merge_call<'a>(
    expr: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> Option<(Node<'a>, Node<'a>, Node<'a>)> {
    let mut curr = expr;
    loop {
        if curr.kind() == "call_expression"
            && let Some(callee) = curr.child(0)
        {
            let is_callee_completed =
                is_completed_callee(callee) || ctx.has_blank_line_between(&callee, &curr);
            let mut cursor = curr.walk();
            for child in curr.children(&mut cursor) {
                if child.kind() == "call_suffix"
                    && child
                        .child(0)
                        .is_some_and(|c| c.kind() == "value_arguments")
                    && ctx.has_newline_between(&callee, &child)
                    && (is_callee_completed || ctx.has_blank_line_between(&callee, &child))
                {
                    return Some((curr, callee, child));
                }
            }
            curr = callee;
        } else if curr.kind() == "navigation_expression"
            && let Some(receiver) = curr.child(0)
        {
            curr = receiver;
        } else {
            break;
        }
    }
    None
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

        let outer_expr = find_statement_expr(child);
        if let Some((merge_call, callee, suffix)) = find_merge_call(outer_expr, ctx)
            && let Some(val_args) = suffix.child(0)
        {
            let doc1 = if child.id() == outer_expr.id() {
                lower_node(callee, ctx)
            } else {
                let ctx1 = ctx.with_leaf_override(outer_expr.id(), callee);
                lower_node(child, &ctx1)
            };
            stmt_docs.push(doc1);
            if ctx.has_blank_line_between(&callee, &suffix) {
                stmt_docs.push(Doc::EmptyLine);
            } else {
                stmt_docs.push(Doc::HardLine);
            }
            let ctx2 = ctx.with_leaf_override(merge_call.id(), val_args);
            let doc2 = lower_node(outer_expr, &ctx2);
            stmt_docs.push(doc2);
            prev_stmt = Some(child);
            continue;
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
            crate::lower::binary::lower_annotated_statement_infix(child, ctx)
        } else {
            lower_node(child, ctx)
        };
        stmt_docs.push(doc);
        prev_stmt = Some(child);
    }

    Doc::Concat(stmt_docs)
}
