use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn collect_binary_elements<'a>(
    node: Node<'a>,
    parent_kind: &str,
    elements: &mut Vec<Node<'a>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == parent_kind {
            collect_binary_elements(child, parent_kind, elements);
        } else {
            elements.push(child);
        }
    }
}

pub fn is_binary_expression_kind(kind: &str) -> bool {
    matches!(
        kind,
        "binary_expression"
            | "additive_expression"
            | "multiplicative_expression"
            | "comparison_expression"
            | "equality_expression"
            | "conjunction_expression"
            | "disjunction_expression"
            | "infix_expression"
            | "elvis_expression"
            | "as_expression"
            | "check_expression"
            | "assignment"
    )
}

pub fn is_binary_operator(node: &Node<'_>) -> bool {
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

fn push_operator_doc<'a>(
    docs: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    state: &mut BinaryLowerState<'a>,
    ctx: &LowerContext<'a, '_>,
) {
    let kind = child.kind();
    if kind == "?:" {
        state.pending_elvis = true;
        state.elvis_newline = state
            .prev_child
            .is_some_and(|p| ctx.has_newline_between(&p, &child));
    } else if state.is_after_comment {
        let op_doc = if docs.last().is_some_and(|d| d.ends_with_newline()) {
            Doc::text(ctx.node_text(&child))
        } else {
            Doc::Concat(vec![Doc::Line, Doc::text(ctx.node_text(&child))])
        };
        docs.push(Doc::continuation_indent(op_doc));
        state.prev_was_operator = true;
    } else {
        docs.push(Doc::space());
        docs.push(Doc::text(ctx.node_text(&child)));
        state.prev_was_operator = true;
    }
}

fn push_comment_doc<'a>(
    docs: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    state: &mut BinaryLowerState<'a>,
    ctx: &LowerContext<'a, '_>,
) {
    if state
        .prev_child
        .is_some_and(|p| !ctx.has_newline_between(&p, &child))
    {
        docs.push(Doc::space());
        let comment_doc = if state.pending_elvis {
            let block_indent = crate::lower::comment::compute_comment_indent(child, ctx);
            let lhs_len = Doc::slice_last_line_len(docs);
            let start_col = block_indent + lhs_len;
            crate::lower::comment::lower_comment_with_col(child, ctx, Some(start_col))
        } else {
            lower_node(child, ctx)
        };
        docs.push(comment_doc);
        state.is_after_comment = docs.last().is_some_and(|d| d.ends_with_newline());
    } else {
        let line_doc = if docs.last().is_some_and(|d| d.ends_with_newline()) {
            if state
                .prev_child
                .is_some_and(|p| ctx.has_blank_line_between(&p, &child))
            {
                Some(Doc::EmptyLine)
            } else {
                None
            }
        } else {
            Some(Doc::Line)
        };
        let comment_doc = match line_doc {
            Some(l) => Doc::Concat(vec![l, lower_node(child, ctx)]),
            None => lower_node(child, ctx),
        };
        docs.push(Doc::continuation_indent(comment_doc));
        state.is_after_comment = true;
        state.prev_was_operator = false;
    }
}

fn push_post_comment_child<'a>(
    docs: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    state: &mut BinaryLowerState<'a>,
    ctx: &LowerContext<'a, '_>,
) {
    if docs.last().is_some_and(|d| d.ends_with_newline()) {
        if state.prev_was_operator {
            let stripped_newline = match docs.last_mut() {
                Some(Doc::Concat(items)) => {
                    if items.last().is_some_and(|d| matches!(d, Doc::HardLine)) {
                        items.pop();
                        true
                    } else {
                        false
                    }
                }
                Some(Doc::HardLine) => {
                    docs.pop();
                    true
                }
                _ => false,
            };
            if stripped_newline {
                docs.push(Doc::continuation_indent(Doc::Concat(vec![
                    Doc::HardLine,
                    lower_node(child, ctx),
                ])));
            } else {
                docs.push(Doc::continuation_indent(lower_node(child, ctx)));
            }
            state.prev_was_operator = false;
        } else {
            docs.push(lower_node(child, ctx));
        }
    } else {
        docs.push(Doc::space());
        docs.push(lower_node(child, ctx));
        state.prev_was_operator = false;
    }
    state.is_after_comment = false;
}

fn push_binary_child_doc<'a>(
    docs: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    prefix_doc: Option<Doc<'a>>,
    state: &mut BinaryLowerState<'a>,
    ctx: &LowerContext<'a, '_>,
) {
    let is_comment = matches!(
        child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let is_operator = is_binary_operator(&child);

    if docs.is_empty() {
        let child_doc = lower_node(child, ctx);
        if let Some(prefix) = prefix_doc {
            docs.push(Doc::Group(Box::new(Doc::Concat(vec![
                prefix,
                Doc::Line,
                child_doc,
            ]))));
        } else {
            docs.push(child_doc);
        }
    } else if is_comment {
        push_comment_doc(docs, child, state, ctx);
    } else if is_operator {
        push_operator_doc(docs, child, state, ctx);
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
    } else if state.is_after_comment {
        push_post_comment_child(docs, child, state, ctx);
    } else if state.prev_was_operator {
        docs.push(Doc::continuation_indent(Doc::Concat(vec![
            Doc::Line,
            lower_node(child, ctx),
        ])));
        state.prev_was_operator = false;
    } else {
        docs.push(Doc::space());
        docs.push(lower_node(child, ctx));
    }
    state.prev_child = Some(child);
}

pub fn lower_binary_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    lower_binary_expression_with_prefix(node, None, ctx)
}

pub fn lower_binary_expression_with_prefix<'a>(
    node: Node<'a>,
    prefix_doc: Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let mut elements = Vec::new();
    collect_binary_elements(node, node.kind(), &mut elements);

    if elements.is_empty() {
        return Doc::empty();
    }

    let mut docs = Vec::new();
    let mut state = BinaryLowerState::default();

    for (idx, child) in elements.into_iter().enumerate() {
        let prefix = if idx == 0 { prefix_doc.clone() } else { None };
        push_binary_child_doc(&mut docs, child, prefix, &mut state, ctx);
    }

    Doc::Group(Box::new(Doc::Concat(docs)))
}

pub fn lower_annotated_statement_infix<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
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
