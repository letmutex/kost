use tree_sitter::Node;

use crate::doc::Doc;
pub use crate::lower::comma_list::{CommaListItem, CommaListOptions, format_comma_list};
use crate::lower::context::LowerContext;
use crate::lower::lower_node;
use crate::lower::navigation::is_call_receiver_in_chain;

pub(crate) fn has_trailing_comma(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    let mut trailing = false;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" || kind == ")" || kind == "[" || kind == "]" || kind == "<" || kind == ">" {
            continue;
        }
        if child.is_missing() || child.byte_range().is_empty() {
            continue;
        }
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            continue;
        }
        trailing = kind == ",";
    }
    trailing
}

pub(crate) fn extract_comma_list_items<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
) -> (Vec<CommaListItem<'a>>, bool) {
    let mut modifier_docs: Vec<Doc<'a>> = Vec::new();
    let mut prefix_docs = Vec::new();
    let mut rhs_docs = Vec::new();
    let mut has_equals = false;
    let mut items: Vec<CommaListItem<'a>> = Vec::new();
    let mut has_trailing_comma = false;
    let mut prev_token: Option<Node<'a>> = None;

    let mut pending_comments: Vec<Doc<'a>> = Vec::new();

    let flush_item = |modifier: &mut Vec<Doc<'a>>,
                      prefix: &mut Vec<Doc<'a>>,
                      rhs: &mut Vec<Doc<'a>>,
                      has_eq: &mut bool,
                      pending: &mut Vec<Doc<'a>>,
                      items: &mut Vec<CommaListItem<'a>>| {
        if !modifier.is_empty() || !prefix.is_empty() || !rhs.is_empty() {
            let mut doc = if *has_eq {
                let p = Doc::Concat(std::mem::take(prefix));
                let r = Doc::Concat(std::mem::take(rhs));
                let eq_group = Doc::Group(Box::new(Doc::Concat(vec![
                    p,
                    Doc::space(),
                    Doc::text("="),
                    Doc::if_break(
                        Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, r.clone()])),
                        Doc::Concat(vec![Doc::space(), r]),
                    ),
                ])));
                if modifier.is_empty() {
                    eq_group
                } else {
                    Doc::Concat(vec![Doc::Concat(std::mem::take(modifier)), eq_group])
                }
            } else {
                let p = Doc::Concat(std::mem::take(prefix));
                if modifier.is_empty() {
                    p
                } else {
                    Doc::Concat(vec![Doc::Concat(std::mem::take(modifier)), p])
                }
            };
            if !pending.is_empty() {
                let mut parts = Vec::new();
                for c in pending.drain(..) {
                    parts.push(c);
                    if !parts.last().is_some_and(|d| d.ends_with_newline()) {
                        parts.push(Doc::HardLine);
                    }
                }
                parts.push(doc);
                doc = Doc::Concat(parts);
            }
            items.push(CommaListItem {
                doc,
                trailing_comment: None,
                is_comment: false,
            });
            *has_eq = false;
        }
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "(" || kind == ")" || kind == "[" || kind == "]" || kind == "<" || kind == ">" {
            prev_token = Some(child);
            continue;
        }
        if child.is_missing() || child.byte_range().is_empty() {
            continue;
        }
        let is_modifier = prev_token.is_some_and(|p| {
            matches!(
                p.kind(),
                "parameter_modifiers" | "modifiers" | "type_parameter_modifiers" | "annotation"
            )
        });
        if matches!(
            kind,
            "parameter_modifiers" | "modifiers" | "type_parameter_modifiers"
        ) {
            let mod_doc = lower_node(child, ctx);
            modifier_docs.push(mod_doc);
            prev_token = Some(child);
            continue;
        }
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if is_modifier {
                let comment_doc = lower_node(child, ctx);
                if !modifier_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
                {
                    modifier_docs.push(Doc::space());
                }
                modifier_docs.push(comment_doc);
                prev_token = Some(child);
                continue;
            }
            let is_same_line =
                prev_token.is_some_and(|p| child.start_position().row == p.end_position().row);
            flush_item(
                &mut modifier_docs,
                &mut prefix_docs,
                &mut rhs_docs,
                &mut has_equals,
                &mut pending_comments,
                &mut items,
            );
            if is_same_line && !items.is_empty() {
                items.last_mut().unwrap().trailing_comment = Some(lower_node(child, ctx));
            } else {
                pending_comments.push(lower_node(child, ctx));
            }
            prev_token = Some(child);
            continue;
        }
        if kind == "," {
            flush_item(
                &mut modifier_docs,
                &mut prefix_docs,
                &mut rhs_docs,
                &mut has_equals,
                &mut pending_comments,
                &mut items,
            );
            has_trailing_comma = true;
            prev_token = Some(child);
            continue;
        }
        has_trailing_comma = false;
        if kind == "=" {
            has_equals = true;
            prev_token = Some(child);
            continue;
        }

        let target = if has_equals {
            &mut rhs_docs
        } else {
            &mut prefix_docs
        };

        if !target.is_empty()
            && !target
                .last()
                .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
        {
            target.push(Doc::space());
        }
        target.push(lower_node(child, ctx));
        prev_token = Some(child);
    }
    flush_item(
        &mut modifier_docs,
        &mut prefix_docs,
        &mut rhs_docs,
        &mut has_equals,
        &mut pending_comments,
        &mut items,
    );
    for c in pending_comments {
        items.push(CommaListItem {
            doc: c,
            trailing_comment: None,
            is_comment: true,
        });
    }
    (items, has_trailing_comma)
}

pub(crate) fn has_newline_after_first_token(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut c = node.walk();
    let mut first = None;
    let mut second = None;
    for child in node.children(&mut c) {
        if first.is_none() {
            first = Some(child);
        } else if second.is_none() {
            second = Some(child);
            break;
        }
    }
    match (first, second) {
        (Some(f), Some(s)) => ctx.has_newline_between(&f, &s),
        _ => false,
    }
}

fn is_single_unnamed_lambda_arg(node: &Node<'_>) -> bool {
    if node.kind() != "value_arguments" {
        return false;
    }
    let mut cursor = node.walk();
    let mut args = node
        .children(&mut cursor)
        .filter(|c| c.kind() == "value_argument");
    let (Some(first), None) = (args.next(), args.next()) else {
        return false;
    };
    let has_name = first
        .children(&mut first.walk())
        .any(|c| c.kind() == "=" || c.kind() == "value_argument_name");
    if has_name {
        return false;
    }
    first
        .children(&mut first.walk())
        .any(|c| matches!(c.kind(), "lambda_literal" | "annotated_lambda"))
}

fn should_break_after_open(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    if node.kind() != "value_arguments" {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    if matches!(parent.kind(), "call_suffix" | "call_expression") {
        let mut prev = node.prev_sibling();
        while let Some(p) = prev {
            if p.kind() == "type_arguments" {
                if ctx.node_text(&p).contains('\n') {
                    return true;
                }
                if ctx.options.block_indent == 4
                    && let Some(call_expr) =
                        parent.parent().filter(|n| n.kind() == "call_expression")
                    && crate::lower::chain::is_chain_broken(
                        &call_expr,
                        ctx,
                        call_expr.byte_range().len(),
                    )
                {
                    return true;
                }
            }
            prev = p.prev_sibling();
        }
    }
    false
}

pub fn lower_list_with_commas<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let is_declaration = matches!(
        node.kind(),
        "function_value_parameters" | "class_parameters"
    );
    let is_open_bracket = matches!(node.kind(), "indexing_suffix" | "collection_literal")
        || (node.kind() == "multi_variable_declaration"
            && node.child(0).is_some_and(|c| c.kind() == "["));
    let (open_token, close_token) = if is_open_bracket {
        ("[", "]")
    } else {
        ("(", ")")
    };

    let has_user_newline = ctx.node_text(&node).contains('\n');
    let has_break_after_open = should_break_after_open(&node, ctx);
    let is_single_lambda = is_single_unnamed_lambda_arg(&node);
    let (items, has_trailing_comma) = extract_comma_list_items(node, ctx);
    let res = format_comma_list(
        items,
        has_trailing_comma,
        open_token,
        close_token,
        CommaListOptions {
            strategy: ctx.options.trailing_comma_strategy,
            is_declaration,
            has_user_newline,
            has_break_after_open,
            is_single_unnamed_lambda: is_single_lambda,
            is_kotlinlang: ctx.options.block_indent == 4,
        },
    );
    if is_call_receiver_in_chain(&node, ctx) {
        Doc::continuation_indent(res)
    } else {
        res
    }
}

pub fn lower_collection_literal<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let has_user_newline = ctx.node_text(&node).contains('\n');
    let has_break_after_open = has_newline_after_first_token(&node, ctx);
    let (items, has_trailing_comma) = extract_comma_list_items(node, ctx);
    format_comma_list(
        items,
        has_trailing_comma,
        "[",
        "]",
        CommaListOptions {
            strategy: ctx.options.trailing_comma_strategy,
            is_declaration: false,
            has_user_newline,
            has_break_after_open,
            is_single_unnamed_lambda: false,
            is_kotlinlang: ctx.options.block_indent == 4,
        },
    )
}
