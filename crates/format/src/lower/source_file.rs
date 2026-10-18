use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::accessors::{is_field_accessor, is_top_level_accessor};
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

fn class_has_body(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "class_body" | "enum_class_body") {
            return true;
        }
    }
    false
}

fn is_orphan_constructor(node: Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    (node.kind() == "call_expression" || node.kind() == "secondary_constructor")
        && ctx.node_text(&node).starts_with("constructor")
}

fn is_comment_before_orphan_constructor(comment: Node<'_>, ctx: &LowerContext<'_, '_>) -> bool {
    let mut next = comment.next_sibling();
    while let Some(n) = next {
        if matches!(n.kind(), "comment" | "line_comment" | "multiline_comment") {
            next = n.next_sibling();
        } else {
            return is_orphan_constructor(n, ctx);
        }
    }
    false
}

fn is_major_top_level_decl(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "class_declaration"
            | "object_declaration"
            | "typealias"
            | "interface_declaration"
    )
}

fn separate_major_top_level_decl<'a>(
    prev: Node<'a>,
    child: Node<'a>,
    docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let is_class_before_constructor = prev.kind() == "class_declaration"
        && !class_has_body(&prev)
        && (is_orphan_constructor(child, ctx) || is_comment_before_orphan_constructor(child, ctx));
    if is_class_before_constructor {
        if !docs.last().is_some_and(|d| d.ends_with_newline()) {
            docs.push(Doc::HardLine);
        }
    } else if prev.kind() == "import_list" && docs.last().is_some_and(|d| d.ends_with_newline()) {
        // Already separated by import_list bottom comments
    } else if matches!(prev.kind(), "annotation" | "parenthesized_expression") {
        if !docs.last().is_some_and(|d| d.ends_with_newline()) {
            docs.push(Doc::HardLine);
        }
    } else {
        docs.push(Doc::EmptyLine);
    }
}

fn separate_source_file_children<'a>(
    prev: Node<'a>,
    child: Node<'a>,
    docs: &mut Vec<Doc<'a>>,
    prev_was_trailing_comment_on_major_decl: &mut bool,
    ctx: &LowerContext<'a, '_>,
) {
    let was_trailing_major = *prev_was_trailing_comment_on_major_decl;
    *prev_was_trailing_comment_on_major_decl = false;

    if was_trailing_major
        && (is_major_top_level_decl(child.kind()) || child.start_position().column > 0)
    {
        docs.push(Doc::EmptyLine);
    } else if prev.kind() == "annotation" && child.kind() == "parenthesized_expression" {
        // Do not separate annotation name and its arguments
    } else if prev.kind() == "import_list"
        && !docs.is_empty()
        && !docs.last().is_some_and(|d| d.ends_with_newline())
    {
        docs.push(Doc::EmptyLine);
    } else if prev.kind() == "package_header"
        && !docs.is_empty()
        && !docs.last().is_some_and(|d| matches!(d, Doc::EmptyLine))
    {
        let child_is_comment = matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        );
        if ctx.has_blank_line_between(&prev, &child)
            || (!child_is_comment && !docs.last().is_some_and(|d| d.ends_with_newline()))
        {
            docs.push(Doc::EmptyLine);
        } else if child_is_comment && !docs.last().is_some_and(|d| d.ends_with_newline()) {
            docs.push(Doc::HardLine);
        }
    } else if ctx.has_blank_line_between(&prev, &child) {
        docs.push(Doc::EmptyLine);
    } else if matches!(prev.kind(), "comment" | "multiline_comment")
        && ctx.node_text(&prev).starts_with("/**")
    {
        if !docs.last().is_some_and(|d| d.ends_with_newline()) {
            docs.push(Doc::HardLine);
        }
    } else if !ctx.has_newline_between(&prev, &child) {
        docs.push(Doc::space());
        if is_major_top_level_decl(prev.kind())
            && matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            *prev_was_trailing_comment_on_major_decl = true;
        }
    } else if !matches!(
        prev.kind(),
        "comment" | "line_comment" | "multiline_comment"
    ) && (is_major_top_level_decl(prev.kind()) || is_major_top_level_decl(child.kind()))
    {
        separate_major_top_level_decl(prev, child, docs, ctx);
    } else if !docs.last().is_some_and(|d| d.ends_with_newline()) {
        docs.push(Doc::HardLine);
    }
}

/// Main lowering entry point converting a Tree-sitter AST root into a Doc IR tree.
pub fn lower_source_file<'a>(root: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = root.walk();
    let mut prev_child: Option<Node<'a>> = None;
    let mut prev_was_trailing_comment_on_major_decl = false;

    for child in root.children(&mut cursor) {
        if child.is_extra()
            && !matches!(
                child.kind(),
                "comment" | "line_comment" | "multiline_comment"
            )
        {
            continue;
        }

        let is_accessor = matches!(child.kind(), "getter" | "setter" | "backing_field")
            || is_field_accessor(&child, ctx)
            || is_top_level_accessor(&child, prev_child, ctx);
        if is_accessor {
            docs.push(Doc::indent(Doc::Concat(vec![
                Doc::HardLine,
                lower_node(child, ctx),
            ])));
        } else {
            if let Some(prev) = prev_child {
                separate_source_file_children(
                    prev,
                    child,
                    &mut docs,
                    &mut prev_was_trailing_comment_on_major_decl,
                    ctx,
                );
            }
            docs.push(lower_node(child, ctx));
        }
        prev_child = Some(child);
    }

    Doc::Concat(docs)
}
