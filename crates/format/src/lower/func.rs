use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::assignment::has_trailing_lambda_child;
use crate::lower::context::LowerContext;
use crate::lower::expr::lower_block;
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
    let mut body_doc = None;
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
            body_doc = Some(lower_node(child, ctx));
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

    let name_group = build_func_name_group(prefix_docs, name_docs, has_dot);
    let header_doc = assemble_func_header(name_group, suffix_docs, where_doc, where_has_newline);
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

    if has_expr_body {
        let header = Doc::Group(Box::new(header_doc));
        Doc::Concat(vec![header, Doc::space(), Doc::Group(Box::new(body))])
    } else if is_empty_block {
        Doc::Group(Box::new(Doc::Concat(vec![header_doc, Doc::space(), body])))
    } else {
        let header = Doc::Group(Box::new(header_doc));
        Doc::Concat(vec![header, Doc::space(), body])
    }
}

fn is_pascal_case_call(node: &Node<'_>, source: &str) -> bool {
    if node.kind() != "call_expression" {
        return false;
    }
    let Some(callee) = node.child(0) else {
        return false;
    };
    let name = match callee.kind() {
        "simple_identifier" => source.get(callee.start_byte()..callee.end_byte()),
        _ => None,
    };
    name.is_some_and(|n| n.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
}

fn has_complex_or_control_receiver(node: &Node<'_>) -> bool {
    let mut curr = Some(*node);
    while let Some(n) = curr {
        if matches!(
            n.kind(),
            "elvis_expression"
                | "binary_expression"
                | "if_expression"
                | "when_expression"
                | "try_expression"
        ) {
            return true;
        }
        curr = n.child(0);
    }
    false
}

pub fn lower_function_body<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut cursor = node.walk();
    let mut is_expression_body = false;
    let mut expr_child = None;
    let mut block_child = None;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" {
            is_expression_body = true;
        } else if is_expression_body {
            expr_child = Some(child);
        } else if kind == "block" {
            block_child = Some(child);
        }
    }

    if is_expression_body {
        let is_pascal_call = expr_child.is_some_and(|c| is_pascal_case_call(&c, ctx.source));
        let expr_doc = expr_child
            .map(|c| lower_node(c, ctx))
            .unwrap_or_else(Doc::empty);
        if is_pascal_call {
            return Doc::Concat(vec![
                Doc::text("="),
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, expr_doc])),
            ]);
        }
        if let Some(e_node) = expr_child {
            if matches!(e_node.kind(), "lambda_literal" | "annotated_lambda") {
                return Doc::Concat(vec![Doc::text("="), Doc::space(), expr_doc]);
            }
            let is_block_like = (e_node.kind() == "call_expression"
                || e_node.kind() == "navigation_expression")
                && (has_trailing_lambda_child(&e_node)
                    || (ctx.options.block_indent == 4 && ctx.node_text(&e_node).contains('\n')));
            if is_block_like && !has_complex_or_control_receiver(&e_node) {
                let rhs_raw = ctx.node_text(&e_node);
                let first_line = rhs_raw.lines().next().unwrap_or("");
                let indent = node.start_position().column;
                let full_first = ctx.node_text(&node).lines().next().unwrap_or("");
                let eq_pos = full_first.find('=').unwrap_or(0);
                let total = indent + eq_pos + 1 + first_line.len();
                if total <= ctx.options.max_width {
                    return Doc::Concat(vec![Doc::text("="), Doc::space(), expr_doc]);
                }
            }
        }
        return Doc::Concat(vec![
            Doc::text("="),
            Doc::if_break(
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, expr_doc.clone()])),
                Doc::Concat(vec![Doc::space(), expr_doc]),
            ),
        ]);
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
        && !matches!(
            target.last(),
            Some(
                Doc::Text("(") | Doc::Text("<") | Doc::Text(" ") | Doc::Text(":") | Doc::Text(": ")
            )
        )
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
