use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

fn has_following_sibling_of_kind(node: &Node<'_>, kind: &str) -> bool {
    let mut sibling = node.next_sibling();
    while let Some(sib) = sibling {
        if sib.kind() == kind {
            return true;
        }
        sibling = sib.next_sibling();
    }
    false
}

fn parameter_trailing_comma_reserve(node: &Node<'_>, ctx: &LowerContext<'_, '_>) -> usize {
    if has_following_sibling_of_kind(node, "parameter") {
        return 1;
    }
    let Some(parent) = node.parent() else {
        return 0;
    };
    if !matches!(
        parent.kind(),
        "function_value_parameters" | "class_parameters"
    ) {
        return 0;
    }
    if ctx.options.trailing_comma_strategy != crate::TrailingCommaStrategy::None
        && ctx.node_text(&parent).contains('\n')
    {
        return 1;
    }
    0
}

pub fn lower_parameter<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut name_docs: Vec<Doc<'a>> = Vec::new();
    let mut type_docs: Vec<Doc<'a>> = Vec::new();
    let mut default_docs: Vec<Doc<'a>> = Vec::new();
    let mut is_after_equals = false;
    let mut is_after_colon = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" {
            is_after_equals = true;
            continue;
        }
        if kind == ":" {
            is_after_colon = true;
            continue;
        }

        if is_after_equals {
            if !default_docs.is_empty()
                && !default_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                default_docs.push(Doc::space());
            }
            default_docs.push(lower_node(child, ctx));
        } else if is_after_colon {
            if !type_docs.is_empty()
                && !type_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                type_docs.push(Doc::space());
            }
            type_docs.push(lower_node(child, ctx));
        } else {
            if !name_docs.is_empty()
                && !name_docs
                    .last()
                    .is_some_and(|d| d.ends_with_space() || d.ends_with_line())
            {
                name_docs.push(Doc::space());
            }
            name_docs.push(lower_node(child, ctx));
        }
    }

    let mut prefix_parts = vec![Doc::Concat(name_docs)];
    if !type_docs.is_empty() {
        let reserve = parameter_trailing_comma_reserve(&node, ctx);
        let mut type_parts = vec![Doc::Line, Doc::Concat(type_docs)];
        if reserve > 0 {
            type_parts.push(Doc::reserve_width(reserve));
        }
        prefix_parts.push(Doc::Group(Box::new(Doc::Concat(vec![
            Doc::text(":"),
            Doc::continuation_indent(Doc::Concat(type_parts)),
        ]))));
    }
    let prefix_doc = Doc::Concat(prefix_parts);

    if is_after_equals {
        let default_doc = Doc::Concat(default_docs);
        Doc::Group(Box::new(Doc::Concat(vec![
            prefix_doc,
            Doc::space(),
            Doc::text("="),
            Doc::if_break(
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, default_doc.clone()])),
                Doc::Concat(vec![Doc::space(), default_doc]),
            ),
        ])))
    } else {
        prefix_doc
    }
}

pub fn lower_multi_variable_declaration<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    crate::lower::lists::lower_list_with_commas(node, ctx)
}

pub fn value_argument_closing_reserve(node: &Node<'_>) -> usize {
    if has_following_sibling_of_kind(node, "value_argument") {
        return 1;
    }
    let Some(parent) = node.parent() else {
        return 0;
    };
    if parent.kind() != "value_arguments" {
        return 0;
    }
    let Some(suffix) = parent.parent() else {
        return 1;
    };
    let Some(call) = suffix.parent() else {
        return 1;
    };
    let Some(arg) = call.parent() else {
        return 1;
    };
    if arg.kind() != "value_argument" {
        return 1;
    }
    let mut count = 1;
    let mut curr = Some(arg);
    while let Some(current_arg) = curr {
        let Some(args) = current_arg.parent() else {
            break;
        };
        if args.kind() != "value_arguments" {
            break;
        }
        if has_following_sibling_of_kind(&current_arg, "value_argument") {
            break;
        }
        count += 1;
        let Some(suf) = args.parent() else {
            break;
        };
        let Some(c_expr) = suf.parent() else {
            break;
        };
        curr = c_expr.parent().filter(|p| p.kind() == "value_argument");
    }
    count
}

pub fn lower_value_argument<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut name = None;
    let mut expr = None;
    let mut expr_node = None;
    let mut has_equals = false;

    let mut equals_node = None;
    let mut comments_after_eq = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" {
            has_equals = true;
            equals_node = Some(child);
        } else if !has_equals {
            name = Some(lower_node(child, ctx));
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            comments_after_eq.push(lower_node(child, ctx));
        } else {
            expr = Some(lower_node(child, ctx));
            expr_node = Some(child);
        }
    }

    if has_equals && let (Some(n), Some(e)) = (name, expr) {
        let has_user_newline = equals_node
            .zip(expr_node)
            .is_some_and(|(eq, ex)| ctx.has_newline_between(&eq, &ex));
        let is_lambda =
            expr_node.is_some_and(|en| matches!(en.kind(), "lambda_literal" | "annotated_lambda"));
        let is_call_or_constructor = expr_node
            .is_some_and(|en| matches!(en.kind(), "call_expression" | "constructor_invocation"));
        let mut prefix = vec![n, Doc::space(), Doc::text("=")];
        for c in comments_after_eq {
            prefix.push(Doc::space());
            prefix.push(c);
        }
        if is_lambda || (is_call_or_constructor && !has_user_newline) {
            prefix.push(Doc::space());
            prefix.push(e);
            Doc::Concat(prefix)
        } else {
            let reserve = value_argument_closing_reserve(&node);
            let mut flat_parts = vec![Doc::space(), e.clone()];
            if reserve > 0 {
                flat_parts.push(Doc::reserve_width(reserve));
            }
            prefix.push(Doc::if_break(
                Doc::continuation_indent(Doc::Concat(vec![Doc::HardLine, e])),
                Doc::Concat(flat_parts),
            ));
            Doc::Group(Box::new(Doc::Concat(prefix)))
        }
    } else {
        let mut docs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            docs.push(lower_node(child, ctx));
        }
        Doc::Concat(docs)
    }
}
