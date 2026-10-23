use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{DelimitedListBuilder, format_assign_like, format_delimited_list_ext};
use crate::lower::context::LowerContext;

#[derive(Default)]
struct FuncDeclParts<'a> {
    modifier_docs: Vec<Doc<'a>>,
    prefix_docs: Vec<Doc<'a>>,
    params_doc: Option<Doc<'a>>,
    return_type_docs: Vec<Doc<'a>>,
    where_doc: Option<Doc<'a>>,
    body_node: Option<Node<'a>>,
    is_expression_body: bool,
}

fn push_func_keyword<'a>(child: &Node<'a>, prefix_docs: &mut Vec<Doc<'a>>) {
    if !prefix_docs.is_empty() && !prefix_docs.last().is_some_and(|d| d.ends_with_newline()) {
        prefix_docs.push(Doc::space());
    }
    let is_direct_params = child
        .next_sibling()
        .is_some_and(|next| next.kind() == "function_value_parameters");
    if is_direct_params {
        prefix_docs.push(Doc::text("fun"));
    } else {
        prefix_docs.push(Doc::text("fun "));
    }
}

fn parse_func_parts<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> FuncDeclParts<'a> {
    let mut parts = FuncDeclParts::default();
    let mut in_return_type = false;

    let mut cursor = node.walk();
    let mut prev_child: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "modifiers" | "annotation" => parts.modifier_docs.push(lower_fn(child, ctx)),
            "context_receiver" | "context_parameters" => {
                crate::rules::comments::push_context_receiver_doc(
                    child,
                    &mut parts.modifier_docs,
                    ctx,
                    lower_fn,
                );
            }
            "comment" | "line_comment" | "multiline_comment" if parts.prefix_docs.is_empty() => {
                crate::rules::comments::push_modifier_comment(
                    child,
                    prev_child,
                    &mut parts.modifier_docs,
                    ctx,
                    lower_fn,
                );
            }
            "fun" => push_func_keyword(&child, &mut parts.prefix_docs),
            "type_parameters" => {
                parts.prefix_docs.push(lower_fn(child, ctx));
                parts.prefix_docs.push(Doc::space());
            }
            "receiver_type" => {
                parts.prefix_docs.push(lower_fn(child, ctx));
            }
            "." => {
                parts
                    .prefix_docs
                    .push(Doc::continuation_indent(Doc::concat(vec![
                        Doc::SoftLine,
                        Doc::text("."),
                    ])));
            }
            "simple_identifier" => parts.prefix_docs.push(Doc::text(ctx.node_text(&child))),
            "function_value_parameters" => {
                parts.params_doc = Some(lower_fn(child, ctx));
            }
            ":" => in_return_type = true,
            "=" => {
                in_return_type = false;
                parts.is_expression_body = true;
            }
            "type_constraints" => {
                in_return_type = false;
                parts.where_doc = Some(lower_fn(child, ctx));
            }
            "function_body" | "block" => {
                in_return_type = false;
                parts.body_node = Some(child);
            }
            _ => {
                if in_return_type {
                    parts.return_type_docs.push(lower_fn(child, ctx));
                } else if parts.is_expression_body && parts.body_node.is_none() {
                    parts.body_node = Some(child);
                } else {
                    parts.prefix_docs.push(lower_fn(child, ctx));
                }
            }
        }
        prev_child = Some(child);
    }
    parts
}

pub fn lower_function_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let parts = parse_func_parts(node, ctx, lower_fn);

    let mut param_and_return = Vec::new();
    if let Some(p) = parts.params_doc {
        param_and_return.push(p);
    }
    if !parts.return_type_docs.is_empty() {
        param_and_return.push(Doc::group(Doc::concat(vec![
            Doc::text(":"),
            Doc::continuation_indent(Doc::concat(vec![
                Doc::Line,
                Doc::concat(parts.return_type_docs),
            ])),
        ])));
    }
    let param_and_return_doc = Doc::group(Doc::concat(param_and_return));

    let mut header = Vec::new();
    header.push(Doc::group(Doc::concat(parts.prefix_docs)));
    header.push(param_and_return_doc);

    if let Some(wh) = parts.where_doc {
        header.push(Doc::continuation_indent(Doc::concat(vec![Doc::Line, wh])));
    }

    let header_doc = Doc::group(Doc::concat(header));
    let func_doc = if let Some(body) = parts.body_node {
        attach_func_body(header_doc, body, ctx, lower_fn)
    } else {
        header_doc
    };

    if !parts.modifier_docs.is_empty() {
        let last_mod_ends_with_line = parts
            .modifier_docs
            .last()
            .is_some_and(|d| d.ends_with_newline() || matches!(d, Doc::HardLine | Doc::EmptyLine));
        if last_mod_ends_with_line {
            Doc::concat(vec![Doc::concat(parts.modifier_docs), func_doc])
        } else {
            Doc::group(Doc::concat(vec![
                Doc::concat(parts.modifier_docs),
                func_doc,
            ]))
        }
    } else {
        func_doc
    }
}

fn attach_func_body<'a>(
    header_doc: Doc<'a>,
    body: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if body.kind() == "function_body" {
        let mut is_expression = false;
        let mut expr_child = None;
        let mut pending_comments = Vec::new();
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            let kind = child.kind();
            if kind == "=" {
                is_expression = true;
            } else if is_expression && expr_child.is_none() {
                if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
                    pending_comments.push(lower_fn(child, ctx));
                } else {
                    expr_child = Some(child);
                }
            }
        }
        if is_expression && let Some(e) = expr_child {
            let rhs_doc = lower_fn(e, ctx);
            let final_rhs = if pending_comments.is_empty() {
                rhs_doc
            } else {
                let mut parts = Vec::new();
                for c in pending_comments {
                    parts.push(c);
                    parts.push(Doc::HardLine);
                }
                parts.push(rhs_doc);
                Doc::concat(parts)
            };
            return format_assign_like(header_doc, "=", e, final_rhs, ctx);
        }
        return Doc::concat(vec![header_doc, Doc::space(), lower_fn(body, ctx)]);
    }

    if body.kind() == "block" {
        Doc::concat(vec![header_doc, Doc::space(), lower_fn(body, ctx)])
    } else {
        let rhs_doc = lower_fn(body, ctx);
        format_assign_like(header_doc, "=", body, rhs_doc, ctx)
    }
}

fn format_accessor_parens<'a>(
    paren_children: Vec<Node<'a>>,
    has_trailing_comma: bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let last_element_idx = paren_children.iter().rposition(|c| {
        !matches!(
            c.kind(),
            "(" | ")"
                | "["
                | "]"
                | "<"
                | ">"
                | ","
                | "comment"
                | "line_comment"
                | "multiline_comment"
        )
    });
    let mut builder = DelimitedListBuilder::new();
    for (i, p_child) in paren_children.into_iter().enumerate() {
        builder.handle_child(p_child, i, last_element_idx, ctx, lower_fn);
    }
    builder.flush(ctx, lower_fn);
    format_delimited_list_ext(
        "(",
        ")",
        builder.items,
        builder.dangling_comments,
        has_trailing_comma,
        false,
        true,
        ctx.options.trailing_comma_strategy,
        true,
        false,
        false,
    )
}

pub fn lower_accessor<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut modifier_docs = Vec::new();
    let mut head_docs = Vec::new();
    let mut in_parens = false;
    let mut paren_children: Vec<Node<'a>> = Vec::new();
    let mut has_trailing_comma = false;
    let mut body_node = None;
    let mut is_after_colon = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "modifiers" {
            modifier_docs.push(lower_fn(child, ctx));
            continue;
        }
        if kind == "(" {
            in_parens = true;
            continue;
        }
        if kind == ")" {
            in_parens = false;
            head_docs.push(format_accessor_parens(
                std::mem::take(&mut paren_children),
                has_trailing_comma,
                ctx,
                lower_fn,
            ));
            continue;
        }
        if in_parens {
            if kind == "," {
                has_trailing_comma = true;
            }
            paren_children.push(child);
            continue;
        }
        if kind == "function_body" || kind == "block" {
            body_node = Some(child);
            continue;
        }
        if kind == ":" {
            is_after_colon = true;
            continue;
        }
        if is_after_colon {
            head_docs.push(Doc::text(": "));
            head_docs.push(lower_fn(child, ctx));
            is_after_colon = false;
            continue;
        }
        if !head_docs.is_empty()
            && !head_docs
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
        {
            head_docs.push(Doc::space());
        }
        head_docs.push(lower_fn(child, ctx));
    }

    let sig_doc = Doc::group(Doc::concat(head_docs));
    let header_doc = if modifier_docs.is_empty() {
        sig_doc
    } else {
        Doc::concat(vec![Doc::concat(modifier_docs), sig_doc])
    };

    let Some(body) = body_node else {
        return header_doc;
    };

    attach_func_body(header_doc, body, ctx, lower_fn)
}
