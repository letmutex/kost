use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{
    DelimitedListBuilder, format_assign_like, format_assign_with_comments,
    format_delimited_list_ext,
};
use crate::lower::context::LowerContext;

/// Lower a parameter or class_parameter node (`name: Type = default`, `val x: Int = 1`)
pub fn lower_parameter<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut prefix_docs = Vec::new();
    let mut is_after_equals = false;
    let mut in_colon = false;
    let mut default_node = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "modifiers" | "parameter_modifiers" => {
                prefix_docs.push(lower_fn(child, ctx));
            }
            "val" | "var" | "binding_pattern_kind" => {
                let kw = ctx.node_text(&child);
                prefix_docs.push(Doc::smol(common::concat_smol_str!(kw, " ")));
            }
            ":" => in_colon = true,
            "=" => is_after_equals = true,
            _ => {
                if is_after_equals && default_node.is_none() {
                    default_node = Some(child);
                } else if in_colon {
                    let type_doc = lower_fn(child, ctx);
                    prefix_docs.push(Doc::group(Doc::concat(vec![
                        Doc::text(":"),
                        Doc::continuation_indent(Doc::concat(vec![Doc::Line, type_doc])),
                    ])));
                    in_colon = false;
                } else {
                    prefix_docs.push(lower_fn(child, ctx));
                }
            }
        }
    }

    let prefix_doc = Doc::concat(prefix_docs);
    if let Some(def) = default_node {
        let rhs_doc = lower_fn(def, ctx);
        format_assign_like(prefix_doc, "=", def, rhs_doc, ctx)
    } else {
        prefix_doc
    }
}

/// Lower a value_argument node (`foo = bar`, `*args`, `expr`)
pub fn lower_value_argument<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut name_node = None;
    let mut op_node = None;
    let mut rhs_node = None;
    let mut comments_before_rhs = Vec::new();
    let mut prefix_nodes = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "=" {
            op_node = Some(child);
        } else if op_node.is_none() && kind == "simple_identifier" {
            name_node = Some(child);
        } else if op_node.is_none() {
            prefix_nodes.push(child);
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            if rhs_node.is_none() {
                comments_before_rhs.push(child);
            }
        } else if rhs_node.is_none() {
            rhs_node = Some(child);
        }
    }

    if let (Some(name), Some(rhs)) = (name_node, rhs_node)
        && op_node.is_some()
    {
        let mut lhs: Vec<Doc<'a>> = prefix_nodes.into_iter().map(|c| lower_fn(c, ctx)).collect();
        lhs.push(lower_fn(name, ctx));
        let lhs_doc = Doc::concat(lhs);
        let rhs_doc = lower_fn(rhs, ctx);
        if comments_before_rhs.is_empty() {
            return format_assign_like(lhs_doc, "=", rhs, rhs_doc, ctx);
        } else {
            return format_assign_with_comments(
                lhs_doc,
                "=",
                op_node,
                &comments_before_rhs,
                rhs,
                rhs_doc,
                ctx,
                lower_fn,
            );
        }
    }

    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
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

/// Lower delimited lists (`(...)`, `[...]`, `<...>`) preserving item boundaries and comments
pub fn lower_delimited_list_node<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let builder = DelimitedListBuilder::build_from_node(node, ctx, lower_fn);
    let has_trailing = builder.has_trailing_comma;
    if !has_trailing && builder.items.len() == 1 && is_single_unnamed_lambda_arg(&node) {
        let item_doc = builder.items.into_iter().next().unwrap().doc;
        return Doc::concat(vec![
            Doc::text(builder.open),
            item_doc,
            Doc::text(builder.close),
        ]);
    }
    let kind = crate::engine::node_kind(node);
    let use_cont = kind != "class_parameters";
    let is_declaration = matches!(
        kind,
        "function_value_parameters" | "class_parameters" | "type_parameters"
    );
    let omit_closing_break = !is_declaration
        && !has_trailing
        && ctx.style.pack_closing_paren_after_block_arg
        && ctx.options.trailing_comma_strategy != crate::options::TrailingCommaStrategy::Complete
        && (ctx.options.trailing_comma_strategy == crate::options::TrailingCommaStrategy::None
            || builder
                .items
                .last()
                .is_some_and(|d| d.doc.ends_with_brace()));

    format_delimited_list_ext(
        builder.open,
        builder.close,
        builder.items,
        builder.dangling_comments,
        has_trailing,
        builder.has_user_newline,
        is_declaration,
        ctx.options.trailing_comma_strategy,
        use_cont,
        omit_closing_break,
        !is_declaration || kind == "type_parameters",
    )
}

pub fn lower_value_arguments_multiline<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let builder = DelimitedListBuilder::build_from_node(node, ctx, lower_fn);
    let has_trailing = builder.has_trailing_comma;
    let mut items = builder.items;
    if let Some(first) = items.first_mut() {
        first.force_newline_after = true;
    }
    let omit_closing_break = !has_trailing
        && ctx.options.trailing_comma_strategy == crate::options::TrailingCommaStrategy::None;
    format_delimited_list_ext(
        builder.open,
        builder.close,
        items,
        builder.dangling_comments,
        has_trailing,
        true,
        false,
        ctx.options.trailing_comma_strategy,
        true,
        omit_closing_break,
        false,
    )
}
