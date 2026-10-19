use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::expr::lower_block;
use crate::lower::expression_body::format_expression_body;
use crate::lower::lower_node;
use crate::lower::types::lower_type_constraints;

pub fn lower_function_declaration<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut context_doc = None;
    let mut modifier_docs = Vec::new();
    let mut prefix_docs = Vec::new();
    let mut name_docs = Vec::new();
    let mut suffix_docs = Vec::new();
    let mut where_doc = None;
    let mut where_has_newline = false;
    let mut has_dot = false;
    let mut after_name = false;
    let mut is_after_colon = false;
    let mut body_child = None;

    let mut cursor = node.walk();
    let mut prev_child_node: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "context_receiver" {
            context_doc = Some(lower_node(child, ctx));
        } else if kind == "modifiers" {
            modifier_docs.push(lower_node(child, ctx));
        } else if kind == "function_body" || kind == "block" {
            body_child = Some(child);
        } else if kind == "type_constraints" {
            where_has_newline =
                prev_child_node.is_some_and(|p| ctx.has_newline_between(&p, &child));
            where_doc = Some(lower_type_constraints(child, ctx));
        } else if kind == "." {
            has_dot = true;
        } else {
            if kind == "function_value_parameters" {
                after_name = true;
            }
            let target = if after_name {
                &mut suffix_docs
            } else if has_dot {
                &mut name_docs
            } else {
                &mut prefix_docs
            };

            push_func_token(target, child, &mut is_after_colon, ctx);
        }
        prev_child_node = Some(child);
    }

    let after_params_len = if suffix_docs.len() > 1 {
        Some(suffix_docs[1..].iter().map(|d| d.flat_len()).sum::<usize>())
    } else if !suffix_docs.is_empty() {
        Some(0)
    } else {
        None
    };
    let has_where = where_doc.is_some();
    let name_group = build_func_name_group(prefix_docs, name_docs, has_dot);
    let header_doc = assemble_func_header(name_group, suffix_docs, where_doc, where_has_newline);

    let (header_col, is_multiline_header) = compute_func_header_col(
        &node,
        &modifier_docs,
        &header_doc,
        after_params_len,
        has_where,
        ctx,
    );

    let body_doc = body_child.map(|b| {
        if b.kind() == "function_body" {
            lower_function_body_with_header(b, ctx, Some(header_col), is_multiline_header)
        } else {
            lower_node(b, ctx)
        }
    });

    let func_doc = attach_func_body(header_doc, body_doc, body_child, ctx);
    wrap_func_modifiers_and_context(func_doc, modifier_docs, context_doc)
}

fn assemble_func_header<'a>(
    name_group: Doc<'a>,
    suffix_docs: Vec<Doc<'a>>,
    where_doc: Option<Doc<'a>>,
    where_has_newline: bool,
) -> Doc<'a> {
    let func_header_inner = Doc::Concat(vec![name_group, Doc::Concat(suffix_docs)]);
    let func_header = if where_doc.is_some() {
        Doc::Group(Box::new(func_header_inner))
    } else {
        func_header_inner
    };

    let mut header_parts = vec![func_header];
    let where_line = if where_has_newline {
        Doc::HardLine
    } else {
        Doc::Line
    };
    if let Some(wh) = where_doc.map(|w| Doc::continuation_indent(Doc::Concat(vec![where_line, w])))
    {
        header_parts.push(wh);
    }
    Doc::Concat(header_parts)
}

fn wrap_func_modifiers_and_context<'a>(
    func_doc: Doc<'a>,
    modifier_docs: Vec<Doc<'a>>,
    context_doc: Option<Doc<'a>>,
) -> Doc<'a> {
    let final_doc = if modifier_docs.is_empty() {
        func_doc
    } else if modifier_docs.iter().any(|d| d.has_hardline()) {
        Doc::Concat(vec![Doc::Concat(modifier_docs), func_doc])
    } else {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::Concat(modifier_docs),
            func_doc,
        ])))
    };

    if let Some(ctx_doc) = context_doc {
        Doc::Concat(vec![ctx_doc, Doc::HardLine, final_doc])
    } else {
        final_doc
    }
}

fn build_func_name_group<'a>(
    prefix_docs: Vec<Doc<'a>>,
    name_docs: Vec<Doc<'a>>,
    has_dot: bool,
) -> Doc<'a> {
    if has_dot {
        Doc::Group(Box::new(Doc::Concat(vec![
            Doc::Concat(prefix_docs),
            Doc::continuation_indent(Doc::Concat(vec![
                Doc::SoftLine,
                Doc::text("."),
                Doc::Concat(name_docs),
            ])),
        ])))
    } else {
        Doc::Concat(prefix_docs)
    }
}

fn attach_func_body<'a>(
    header_doc: Doc<'a>,
    body_doc: Option<Doc<'a>>,
    body_child: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let Some(body) = body_doc else {
        return Doc::Group(Box::new(header_doc));
    };

    let has_expr_body = body_child.is_some_and(|b| {
        let mut c = b.walk();
        b.children(&mut c).any(|ch| ch.kind() == "=")
    });

    let is_empty_block = body_child.is_some_and(|b| {
        let text = ctx.node_text(&b).trim();
        text == "{}" || text == "{ }"
    });

    if is_empty_block {
        Doc::Group(Box::new(Doc::Concat(vec![header_doc, Doc::space(), body])))
    } else {
        let header = Doc::Group(Box::new(header_doc));
        let body_doc = if has_expr_body {
            Doc::Group(Box::new(body))
        } else {
            body
        };
        Doc::Concat(vec![header, Doc::space(), body_doc])
    }
}

pub fn lower_function_body<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    lower_function_body_with_header(node, ctx, None, false)
}

pub fn lower_function_body_with_header<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    header_col: Option<usize>,
    is_multiline_header: bool,
) -> Doc<'a> {
    let mut cursor = node.walk();
    let mut is_expression_body = false;
    let mut expr_child = None;
    let mut block_child = None;

    let mut comments_before_expr = Vec::new();
    let mut comments_after_expr = Vec::new();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" {
            is_expression_body = true;
        } else if is_expression_body {
            if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                if expr_child.is_none() {
                    comments_before_expr.push(child);
                } else {
                    comments_after_expr.push(child);
                }
            } else if expr_child.is_none() {
                expr_child = Some(child);
            }
        } else if kind == "block" {
            block_child = Some(child);
        }
    }

    if is_expression_body {
        return format_expression_body(
            node,
            expr_child,
            comments_before_expr,
            comments_after_expr,
            ctx,
            header_col,
            is_multiline_header,
        );
    }

    if let Some(b) = block_child {
        return lower_node(b, ctx);
    }

    lower_block(node, ctx)
}

fn push_func_token<'a>(
    target: &mut Vec<Doc<'a>>,
    child: Node<'a>,
    is_after_colon: &mut bool,
    ctx: &LowerContext<'a, '_>,
) {
    let kind = child.kind();
    if !*is_after_colon
        && !target.is_empty()
        && !matches!(
            kind,
            ":" | ";" | "(" | ")" | "<" | ">" | "function_value_parameters"
        )
        && !matches!(target.last(), Some(Doc::Text("(" | "<" | " " | ":" | ": ")))
        && !target
            .last()
            .is_some_and(|d| d.ends_with_line() || d.ends_with_space())
    {
        target.push(Doc::space());
    }

    if kind == ":" {
        *is_after_colon = true;
    } else if *is_after_colon {
        let return_type_doc = lower_node(child, ctx);
        target.push(Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text(":"),
            Doc::continuation_indent(Doc::Concat(vec![Doc::Line, return_type_doc])),
        ]))));
        *is_after_colon = false;
    } else {
        target.push(lower_node(child, ctx));
    }
}

fn compute_func_header_col<'a>(
    node: &Node<'a>,
    modifier_docs: &[Doc<'a>],
    header_doc: &Doc<'a>,
    after_params_len: Option<usize>,
    has_where: bool,
    ctx: &LowerContext<'a, '_>,
) -> (usize, bool) {
    let indent = ctx.compute_target_indent(node);
    let mod_len = Doc::slice_last_line_len(modifier_docs);
    let flat_header_len = indent + mod_len + header_doc.flat_len();
    let is_multiline_header = header_doc.has_hardline() || flat_header_len > ctx.options.max_width;
    let col = if is_multiline_header
        && !has_where
        && let Some(after_params) = after_params_len
    {
        indent + 1 + after_params
    } else {
        indent + mod_len + header_doc.last_line_len()
    };
    (col, is_multiline_header)
}
