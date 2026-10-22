use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{ListItem, format_delimited_list_ext};
use crate::lower::context::LowerContext;

pub fn primary_constructor_has_constructor_keyword(
    node: &Node<'_>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            return true;
        }
        if child.kind() == "(" {
            let prefix_text = ctx
                .source
                .get(node.start_byte()..child.start_byte())
                .unwrap_or("");
            return prefix_text
                .split_whitespace()
                .any(|token| token == "constructor");
        }
    }
    false
}

fn handle_primary_constructor_param<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    param_items: &mut Vec<ListItem<'a>>,
    pending_comments: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
        let is_same_line =
            prev_child.is_some_and(|p| p.end_position().row == child.start_position().row);
        if is_same_line && !param_items.is_empty() {
            let last = param_items.last_mut().unwrap();
            if prev_child.is_some_and(|p| p.kind() == ",") {
                let c_doc = lower_fn(child, ctx);
                last.same_line_comment = Some(Doc::concat(vec![Doc::space(), c_doc]));
                if kind == "line_comment" {
                    last.force_newline_after = true;
                }
            } else if kind != "line_comment" {
                last.doc = Doc::concat(vec![last.doc.clone(), Doc::space(), lower_fn(child, ctx)]);
            } else {
                pending_comments.push(lower_fn(child, ctx));
            }
        } else {
            pending_comments.push(lower_fn(child, ctx));
        }
    } else if kind != "," {
        let item_doc = lower_fn(child, ctx);
        let doc = if pending_comments.is_empty() {
            item_doc
        } else {
            let mut parts = Vec::new();
            for c in pending_comments.drain(..) {
                parts.push(c);
                parts.push(Doc::HardLine);
            }
            parts.push(item_doc);
            Doc::concat(parts)
        };
        param_items.push(ListItem::new(doc));
    }
}

fn assemble_primary_constructor_doc<'a>(
    has_constructor: bool,
    prefix_docs: Vec<Doc<'a>>,
    params_doc: Doc<'a>,
) -> Doc<'a> {
    if has_constructor {
        let mut head = prefix_docs;
        if !head.is_empty()
            && !head
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_newline())
        {
            head.push(Doc::space());
        }
        head.push(Doc::text("constructor"));
        head.push(params_doc);
        Doc::group(Doc::concat(head))
    } else {
        params_doc
    }
}

pub fn lower_primary_constructor<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let has_constructor = primary_constructor_has_constructor_keyword(&node, ctx);
    let mut prefix_docs = Vec::new();
    let mut param_items: Vec<ListItem<'a>> = Vec::new();
    let mut pending_comments = Vec::new();
    let mut in_params = false;
    let mut prev_child: Option<Node<'a>> = None;
    let mut open_p = None;
    let mut close_p = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "modifiers" | "annotation" => {
                prefix_docs.push(lower_fn(child, ctx));
            }
            "(" => {
                in_params = true;
                open_p = Some(child);
            }
            ")" => {
                in_params = false;
                close_p = Some(child);
            }
            _ => {
                if in_params {
                    handle_primary_constructor_param(
                        child,
                        prev_child,
                        &mut param_items,
                        &mut pending_comments,
                        ctx,
                        lower_fn,
                    );
                } else {
                    prefix_docs.push(lower_fn(child, ctx));
                }
            }
        }
        prev_child = Some(child);
    }

    let has_trailing = ctx.has_trailing_comma(&node);
    let has_user_newline = open_p
        .zip(close_p)
        .is_some_and(|(o, c)| ctx.has_user_newline_between(&o, &c));
    let params_doc = format_delimited_list_ext(
        "(",
        ")",
        param_items,
        pending_comments,
        has_trailing,
        has_user_newline,
        true,
        ctx.options.trailing_comma_strategy,
        true,
        false,
        true,
    );

    assemble_primary_constructor_doc(has_constructor, prefix_docs, params_doc)
}

pub fn lower_constructor_delegation_call<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
}

fn format_secondary_constructor_delegation<'a>(
    delegation: Doc<'a>,
    colon_node: Option<Node<'a>>,
    delegation_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let colon_has_newline = match (colon_node, delegation_node) {
        (Some(colon), Some(del)) => ctx.has_user_newline_between(&colon, &del),
        _ => false,
    };
    if colon_has_newline {
        Doc::concat(vec![
            Doc::space(),
            Doc::text(":"),
            Doc::continuation_indent(Doc::concat(vec![Doc::HardLine, delegation])),
        ])
    } else {
        Doc::concat(vec![Doc::space(), Doc::text(": "), delegation])
    }
}

fn assemble_secondary_constructor<'a>(
    head_doc: Doc<'a>,
    has_braces: bool,
    statements_node: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if !has_braces {
        head_doc
    } else if let Some(stmts) = statements_node {
        Doc::concat(vec![
            head_doc,
            Doc::space(),
            Doc::text("{"),
            Doc::indent(Doc::concat(vec![Doc::HardLine, lower_fn(stmts, ctx)])),
            Doc::HardLine,
            Doc::text("}"),
        ])
    } else {
        Doc::concat(vec![head_doc, Doc::space(), Doc::text("{}")])
    }
}

fn handle_secondary_constructor_child<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    modifier_docs: &mut Vec<Doc<'a>>,
    prefix_docs: &mut Vec<Doc<'a>>,
    params_doc: &mut Option<Doc<'a>>,
    delegation_doc: &mut Option<Doc<'a>>,
    colon_node: &mut Option<Node<'a>>,
    delegation_node: &mut Option<Node<'a>>,
    statements_node: &mut Option<Node<'a>>,
    has_braces: &mut bool,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let kind = child.kind();
    match kind {
        "context_receiver" => {
            modifier_docs.push(lower_fn(child, ctx));
            modifier_docs.push(Doc::HardLine);
        }
        "modifiers" => {
            modifier_docs.push(lower_fn(child, ctx));
        }
        "constructor" => {
            prefix_docs.push(Doc::text("constructor"));
        }
        "function_value_parameters" => {
            *params_doc = Some(lower_fn(child, ctx));
        }
        ":" => {
            *colon_node = Some(child);
        }
        "constructor_delegation_call" => {
            *delegation_node = Some(child);
            *delegation_doc = Some(lower_constructor_delegation_call(child, ctx, lower_fn));
        }
        "{" => {
            *has_braces = true;
        }
        "statements" if child.named_child_count() > 0 => {
            *statements_node = Some(child);
        }
        "comment" | "line_comment" | "multiline_comment" => {
            let is_same_line =
                prev_child.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
            if is_same_line {
                if !prefix_docs.last().is_some_and(|d| d.ends_with_space())
                    && !modifier_docs.last().is_some_and(|d| d.ends_with_space())
                {
                    prefix_docs.push(Doc::space());
                }
            } else {
                prefix_docs.push(Doc::HardLine);
            }
            prefix_docs.push(lower_fn(child, ctx));
        }
        _ => {}
    }
}

pub fn lower_secondary_constructor<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut modifier_docs = Vec::new();
    let mut prefix_docs = Vec::new();
    let mut params_doc = None;
    let mut delegation_doc = None;
    let mut colon_node = None;
    let mut delegation_node = None;
    let mut statements_node = None;
    let mut has_braces = false;
    let mut prev_child = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        handle_secondary_constructor_child(
            child,
            prev_child,
            &mut modifier_docs,
            &mut prefix_docs,
            &mut params_doc,
            &mut delegation_doc,
            &mut colon_node,
            &mut delegation_node,
            &mut statements_node,
            &mut has_braces,
            ctx,
            lower_fn,
        );
        prev_child = Some(child);
    }

    let mut header_parts = prefix_docs;
    if let Some(params) = params_doc {
        header_parts.push(params);
    }
    let header = Doc::concat(header_parts);

    let delegation_part = delegation_doc.map(|delegation| {
        format_secondary_constructor_delegation(delegation, colon_node, delegation_node, ctx)
    });

    let mut all_head = vec![header];
    if let Some(del) = delegation_part {
        all_head.push(del);
    }
    let sig_doc = Doc::group(Doc::concat(all_head));
    let head_doc = if modifier_docs.is_empty() {
        sig_doc
    } else {
        let combined = Doc::concat(vec![Doc::concat(modifier_docs), sig_doc]);
        if combined.has_hardline() {
            combined
        } else {
            Doc::group(combined)
        }
    };

    assemble_secondary_constructor(head_doc, has_braces, statements_node, ctx, lower_fn)
}
