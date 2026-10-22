use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::cst_norm::{is_comment_before_orphan_constructor, is_orphan_constructor};

fn class_has_body(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "class_body" | "enum_class_body") {
            return true;
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

fn separate_package_header<'a>(
    prev: Node<'a>,
    child: Node<'a>,
    docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    let last_ends_with_nl = docs.last().is_some_and(|d| d.ends_with_newline());
    if matches!(
        child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    ) && !ctx.has_blank_line_between(&prev, &child)
    {
        if !last_ends_with_nl {
            docs.push(Doc::HardLine);
        }
    } else if ctx.has_blank_line_between(&prev, &child) {
        if last_ends_with_nl {
            docs.push(Doc::HardLine);
        } else {
            docs.push(Doc::EmptyLine);
        }
    } else if !last_ends_with_nl {
        docs.push(Doc::EmptyLine);
    }
}

fn separate_source_file_children<'a>(
    prev: Node<'a>,
    child: Node<'a>,
    docs: &mut Vec<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    if prev.kind() == "annotation" && child.kind() == "parenthesized_expression" {
        return;
    }

    if prev.kind() == "import_list" {
        if !docs.is_empty() && !docs.last().is_some_and(|d| d.ends_with_newline()) {
            docs.push(Doc::EmptyLine);
        }
        return;
    }

    if prev.kind() == "package_header" {
        separate_package_header(prev, child, docs, ctx);
        return;
    }

    let is_class_before_constructor = prev.kind() == "class_declaration"
        && !class_has_body(&prev)
        && (is_orphan_constructor(child, ctx) || is_comment_before_orphan_constructor(child, ctx));

    let is_comment = matches!(
        child.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let prev_is_comment = matches!(
        prev.kind(),
        "comment" | "line_comment" | "multiline_comment"
    );
    let prev_decl = prev.prev_sibling();
    let prev_is_trailing_on_bodiless_class = prev_is_comment
        && prev_decl.is_some_and(|p| {
            !ctx.has_user_newline_between(&p, &prev)
                && p.kind() == "class_declaration"
                && !class_has_body(&p)
        });
    let prev_is_trailing =
        prev_is_comment && prev_decl.is_some_and(|p| !ctx.has_user_newline_between(&p, &prev));

    if is_class_before_constructor {
        if !docs.last().is_some_and(|d| d.ends_with_newline()) {
            docs.push(Doc::HardLine);
        }
    } else if is_comment {
        if !ctx.has_user_newline_between(&prev, &child) {
            docs.push(Doc::space());
        } else if ctx.has_blank_line_between(&prev, &child) || prev_is_trailing_on_bodiless_class {
            docs.push(Doc::EmptyLine);
        } else {
            docs.push(Doc::HardLine);
        }
    } else if prev_is_comment {
        if ctx.has_blank_line_between(&prev, &child)
            || (prev_is_trailing && is_major_top_level_decl(child.kind()))
        {
            docs.push(Doc::EmptyLine);
        } else {
            docs.push(Doc::HardLine);
        }
    } else if is_major_top_level_decl(prev.kind())
        || is_major_top_level_decl(child.kind())
        || ctx.has_blank_line_between(&prev, &child)
    {
        docs.push(Doc::EmptyLine);
    } else {
        docs.push(Doc::HardLine);
    }
}

pub fn lower_source_file<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut prev_child: Option<Node<'a>> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == ";" {
            continue;
        }

        if let Some(prev) = prev_child {
            if crate::rules::comments::try_merge_continuation_comment(
                Some(prev),
                &child,
                &mut docs,
                ctx,
            ) {
                prev_child = Some(child);
                continue;
            }
            let is_accessor = matches!(kind, "getter" | "setter" | "backing_field")
                || (prev.kind() == "property_declaration"
                    && (kind == "call_expression"
                        && child.child(0).is_some_and(|c| {
                            let t = ctx.node_text(&c);
                            t == "get" || t == "set"
                        })))
                || (prev.kind() == "property_declaration"
                    && kind == "assignment"
                    && child.child(0).is_some_and(|c| ctx.node_text(&c) == "field"));

            if is_accessor {
                docs.push(Doc::indent(Doc::concat(vec![
                    Doc::HardLine,
                    lower_fn(child, ctx),
                ])));
                prev_child = Some(child);
                continue;
            }

            separate_source_file_children(prev, child, &mut docs, ctx);
        }

        let child_doc = if kind == "line_comment"
            && prev_child
                .as_ref()
                .is_some_and(|p| !ctx.has_user_newline_between(p, &child))
        {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        docs.push(child_doc);
        prev_child = Some(child);
    }

    Doc::concat(docs)
}
