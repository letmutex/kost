use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

#[derive(Clone)]
struct ModifierContext {
    is_param: bool,
    is_accessor: bool,
    is_class_member_prop: bool,
    is_top_decl: bool,
    should_force_break_in_kotlinlang: bool,
    only_annotations_so_far: bool,
    annotation_count: usize,
}

fn annotation_trailing_doc<'a>(
    child: &Node<'a>,
    node: &Node<'a>,
    ctx: &LowerContext<'a, '_>,
    m_ctx: &ModifierContext,
) -> Doc<'a> {
    let next_node = child.next_sibling().or_else(|| node.next_sibling());
    let has_newline = next_node.is_some_and(|n| ctx.has_newline_between(child, &n));
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
    } else if ctx.options.block_indent == 4 {
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
            && ctx.options.block_indent == 2
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

pub fn lower_modifiers<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
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

    let m_ctx = ModifierContext {
        is_param: matches!(parent_kind, Some("parameter" | "class_parameter")),
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
        should_force_break_in_kotlinlang,
        only_annotations_so_far: true,
        annotation_count: node
            .children(&mut node.walk())
            .filter(|c| c.kind() == "annotation")
            .count(),
    };

    let mut only_annotations_so_far = true;
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        let doc = lower_node(child, ctx);
        if kind == "annotation" {
            let mut item_m_ctx = m_ctx.clone();
            item_m_ctx.only_annotations_so_far = only_annotations_so_far;
            docs.push(doc);
            docs.push(annotation_trailing_doc(&child, &node, ctx, &item_m_ctx));
        } else if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            docs.push(doc);
            let next_node = child.next_sibling().or_else(|| node.next_sibling());
            if next_node.is_some_and(|n| ctx.has_newline_between(&child, &n)) {
                if !docs.last().is_some_and(|d| d.ends_with_newline()) {
                    docs.push(Doc::HardLine);
                }
            } else {
                docs.push(Doc::space());
            }
        } else {
            only_annotations_so_far = false;
            docs.push(doc);
            docs.push(Doc::space());
        }
    }
    Doc::Concat(docs)
}

pub fn lower_file_annotation<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(kind, "comment" | "line_comment" | "multiline_comment") {
            docs.push(Doc::space());
            docs.push(lower_node(child, ctx));
        } else {
            docs.push(lower_node(child, ctx));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_annotation<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
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
        docs.push(lower_node(child, ctx));
        prev_was_entry = inside_brackets;
    }
    Doc::Concat(docs)
}
