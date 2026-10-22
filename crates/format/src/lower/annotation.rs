use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;

#[derive(Clone)]
struct ModifierContext {
    is_param: bool,
    is_accessor: bool,
    is_class_member_prop: bool,
    is_top_decl: bool,
    is_primary_constructor_multiline: bool,
    is_empty_decl: bool,
    should_force_break_in_kotlinlang: bool,
    only_annotations_so_far: bool,
    annotation_count: usize,
}

fn is_empty_declaration(node: &Node<'_>) -> bool {
    if node
        .children(&mut node.walk())
        .any(|c| matches!(c.kind(), "context_receiver" | "context_parameters"))
    {
        return false;
    }
    let kind = node.kind();
    if matches!(kind, "class_declaration" | "object_declaration") {
        let body = node
            .children(&mut node.walk())
            .find(|c| matches!(c.kind(), "class_body" | "enum_class_body"));
        return body.is_none_or(|b| {
            !b.children(&mut b.walk())
                .any(|c| c.kind() != "{" && c.kind() != "}")
        });
    }
    if kind == "function_declaration" {
        let body = node
            .children(&mut node.walk())
            .find(|c| matches!(c.kind(), "function_body" | "block"));
        return body.is_none_or(|b| {
            !b.children(&mut b.walk())
                .any(|c| c.kind() != "{" && c.kind() != "}")
        });
    }
    false
}

fn resolve_annotation_style_trailing_doc<'a>(
    m_ctx: &ModifierContext,
    has_newline: bool,
    has_args: bool,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    if m_ctx.is_primary_constructor_multiline {
        if !m_ctx.only_annotations_so_far {
            return Doc::space();
        }
        return Doc::HardLine;
    }
    if ctx.options.use_experimental_engine
        && ctx.style.break_after_declaration_annotations
        && m_ctx.should_force_break_in_kotlinlang
        && m_ctx.only_annotations_so_far
    {
        return Doc::HardLine;
    }
    if m_ctx.is_empty_decl {
        if m_ctx.annotation_count > 1 && has_newline {
            Doc::HardLine
        } else {
            Doc::Line
        }
    } else if ctx.style.break_after_declaration_annotations {
        if !m_ctx.only_annotations_so_far {
            Doc::space()
        } else if m_ctx.should_force_break_in_kotlinlang {
            Doc::HardLine
        } else if m_ctx.is_param {
            if has_newline {
                Doc::HardLine
            } else {
                Doc::space()
            }
        } else if has_newline {
            Doc::HardLine
        } else {
            Doc::space()
        }
    } else if m_ctx.is_param {
        if has_newline {
            Doc::HardLine
        } else {
            Doc::space()
        }
    } else if m_ctx.is_accessor
        || (m_ctx.is_class_member_prop
            && !ctx.style.break_after_declaration_annotations
            && m_ctx.annotation_count == 1
            && !has_args)
    {
        Doc::space()
    } else if m_ctx.is_top_decl {
        if has_newline {
            Doc::HardLine
        } else {
            Doc::Line
        }
    } else if has_newline {
        Doc::HardLine
    } else {
        Doc::space()
    }
}

fn annotation_trailing_doc<'a>(
    child: &Node<'a>,
    node: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
    m_ctx: &ModifierContext,
) -> Doc<'a> {
    if let Some(n) = child.next_sibling()
        && matches!(n.kind(), "comment" | "line_comment" | "multiline_comment")
    {
        if ctx.has_user_newline_between(child, &n) {
            return Doc::HardLine;
        } else {
            return Doc::space();
        }
    }
    if child.next_sibling().is_none()
        && let Some(n) = node.next_sibling()
        && matches!(n.kind(), "comment" | "line_comment" | "multiline_comment")
    {
        if ctx.has_user_newline_between(node, &n) {
            return Doc::HardLine;
        } else {
            return Doc::space();
        }
    }
    let next_node = child.next_sibling().or_else(|| node.next_sibling());
    let has_newline = next_node.is_some_and(|n| ctx.has_user_newline_between(child, &n));
    let has_args = child.children(&mut child.walk()).any(|c| {
        matches!(
            c.kind(),
            "value_arguments" | "type_arguments" | "[" | "constructor_invocation"
        )
    });
    let has_use_site = child
        .children(&mut child.walk())
        .any(|c| c.kind() == "use_site_target");
    let has_bracket = child.children(&mut child.walk()).any(|c| c.kind() == "[");

    if has_bracket && node.kind() != "type_modifiers" {
        return Doc::HardLine;
    }

    if node.kind() == "type_modifiers" {
        if has_newline || (m_ctx.is_param && has_use_site) {
            Doc::HardLine
        } else if has_args {
            Doc::Line
        } else {
            Doc::space()
        }
    } else {
        resolve_annotation_style_trailing_doc(m_ctx, has_newline, has_args, ctx)
    }
}

fn is_multiline_constructor(
    parent_kind: Option<&str>,
    parent: &Option<Node<'_>>,
    ctx: &LowerContext<'_, '_>,
) -> bool {
    parent_kind == Some("primary_constructor")
        && parent.is_some_and(|p| {
            ctx.node_text(&p).contains('\n')
                || p.start_position().column + p.byte_range().len() > ctx.options.max_width
                || p.parent().is_some_and(|cls| {
                    ctx.node_text(&cls).contains('\n')
                        || cls.start_position().column + cls.byte_range().len()
                            > ctx.options.max_width
                })
        })
}

fn build_modifier_context<'a>(node: &Node<'a>, ctx: &LowerContext<'a, '_>) -> ModifierContext {
    let parent = node.parent();
    let parent_kind = parent.as_ref().map(|p| p.kind());
    let grandparent_kind = parent.as_ref().and_then(|p| p.parent()).map(|gp| gp.kind());

    let should_force_break_in_kotlinlang = matches!(
        parent_kind,
        Some(
            "class_declaration"
                | "object_declaration"
                | "enum_entry"
                | "companion_object"
                | "property_declaration"
                | "getter"
                | "setter"
                | "secondary_constructor"
                | "function_declaration"
        )
    );

    ModifierContext {
        is_param: matches!(
            parent_kind,
            Some("parameter" | "class_parameter" | "function_value_parameters")
        ),
        is_accessor: matches!(parent_kind, Some("getter" | "setter")),
        is_class_member_prop: parent_kind == Some("property_declaration")
            && matches!(grandparent_kind, Some("class_body" | "enum_class_body")),
        is_top_decl: matches!(
            parent_kind,
            Some("class_declaration" | "object_declaration" | "secondary_constructor")
        ) || (matches!(
            parent_kind,
            Some("function_declaration" | "property_declaration")
        ) && matches!(
            grandparent_kind,
            Some("source_file" | "class_body" | "enum_class_body")
        )),
        is_primary_constructor_multiline: is_multiline_constructor(parent_kind, &parent, ctx),
        is_empty_decl: parent.as_ref().is_some_and(is_empty_declaration),
        should_force_break_in_kotlinlang,
        only_annotations_so_far: true,
        annotation_count: node
            .children(&mut node.walk())
            .filter(|c| c.kind() == "annotation")
            .count(),
    }
}

pub fn lower_modifiers<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let m_ctx = build_modifier_context(&node, ctx);

    let mut only_annotations_so_far = true;
    let mut prev_child: Option<Node<'a>> = None;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if crate::rules::comments::try_merge_continuation_comment(
            prev_child, &child, &mut docs, ctx,
        ) {
            prev_child = Some(child);
            continue;
        }
        let doc = if kind == "line_comment"
            && prev_child.is_some_and(|p| !ctx.has_user_newline_between(&p, &child))
        {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        if kind == "annotation" {
            let mut item_m_ctx = m_ctx.clone();
            item_m_ctx.only_annotations_so_far = only_annotations_so_far;
            docs.push(doc);
            docs.push(annotation_trailing_doc(&child, &node, ctx, &item_m_ctx));
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            docs.push(doc);
            if kind == "line_comment" {
                if !docs.last().is_some_and(|d| d.ends_with_newline()) {
                    docs.push(Doc::HardLine);
                }
            } else {
                let next_node = child.next_sibling().or_else(|| node.next_sibling());
                if next_node.is_some_and(|n| ctx.has_user_newline_between(&child, &n)) {
                    if !docs.last().is_some_and(|d| d.ends_with_newline()) {
                        docs.push(Doc::HardLine);
                    }
                } else {
                    docs.push(Doc::space());
                }
            }
        } else {
            only_annotations_so_far = false;
            docs.push(doc);
            docs.push(Doc::space());
        }
        prev_child = Some(child);
    }
    Doc::concat(docs)
}

pub fn lower_file_annotation<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            docs.push(Doc::space());
            docs.push(lower_fn(child, ctx));
        } else {
            docs.push(lower_fn(child, ctx));
        }
    }
    Doc::concat(docs)
}

pub fn lower_annotation<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    if node.child_count() == 0 {
        return Doc::text(ctx.node_text(&node));
    }
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    let mut inside_brackets = false;
    let mut prev_was_entry = false;

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "[" {
            inside_brackets = true;
            docs.push(Doc::text("["));
            prev_was_entry = false;
            continue;
        }
        if kind == "]" {
            inside_brackets = false;
            docs.push(Doc::text("]"));
            prev_was_entry = false;
            continue;
        }
        if inside_brackets && prev_was_entry {
            docs.push(Doc::space());
        }
        docs.push(lower_fn(child, ctx));
        prev_was_entry = inside_brackets;
    }
    Doc::concat(docs)
}
