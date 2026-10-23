use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::builders::{BlockItem, format_braced_block_ext};
use crate::lower::context::LowerContext;
use crate::options::TrailingCommaStrategy;

fn has_property_accessor(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "getter" | "setter") {
            return true;
        }
    }
    false
}

fn is_heavy_member(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "class_declaration"
            | "object_declaration"
            | "anonymous_initializer"
            | "secondary_constructor"
            | "companion_object"
    )
}

fn requires_blank_line_separation(prev: &Node<'_>, next: &Node<'_>) -> bool {
    if matches!(
        prev.kind(),
        "comment" | "line_comment" | "multiline_comment"
    ) || matches!(
        next.kind(),
        "getter" | "setter" | "comment" | "line_comment" | "multiline_comment"
    ) {
        return false;
    }
    if prev.kind() == ";" {
        return is_heavy_member(next.kind()) || next.kind() == "property_declaration";
    }
    is_heavy_member(prev.kind())
        || is_heavy_member(next.kind())
        || matches!(prev.kind(), "getter" | "setter")
        || (prev.kind() == "property_declaration" && has_property_accessor(prev))
}

fn handle_class_body_comment<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    brace_node: Option<Node<'a>>,
    items: &mut [BlockItem<'a>],
    opening_comment: &mut Option<Doc<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> bool {
    let kind = child.kind();
    if ctx.style.preset == crate::options::StylePreset::Kotlinlang
        && ctx.style.align_wrapped_trailing_comments
        && kind == "line_comment"
        && let Some(prev) = prev_child.and_then(crate::rules::comments::find_trailing_line_comment)
        && crate::rules::comments::should_merge_wrapped_comment(&prev, &child, ctx)
    {
        let text = ctx.node_text(&child);
        if let Some(last) = items.last_mut() {
            if let Some(ref mut slc) = last.same_line_comment
                && crate::rules::comments::try_append_to_trailing_line_comment(slc, text)
            {
                return true;
            }
            if crate::rules::comments::try_append_to_trailing_line_comment(&mut last.doc, text) {
                return true;
            }
        } else if let Some(ref mut oc) = *opening_comment
            && crate::rules::comments::try_append_to_trailing_line_comment(oc, text)
        {
            return true;
        }
    }
    let is_same_line = prev_child.is_some_and(|p| !ctx.has_user_newline_between(&p, &child));
    if is_same_line && let Some(last) = items.last_mut() {
        let c_doc = if kind == "line_comment" {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        last.same_line_comment = Some(Doc::concat(vec![Doc::space(), c_doc]));
        return true;
    }
    if items.is_empty()
        && opening_comment.is_none()
        && brace_node.is_some_and(|b| !ctx.has_user_newline_between(&b, &child))
    {
        let c_doc = if kind == "line_comment" {
            Doc::line_comment(
                ctx.node_text(&child),
                true,
                ctx.style.align_wrapped_trailing_comments,
            )
        } else {
            lower_fn(child, ctx)
        };
        *opening_comment = Some(c_doc);
        return true;
    }
    false
}

fn handle_class_body_semicolon<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    items: &mut Vec<BlockItem<'a>>,
    ctx: &LowerContext<'a, '_>,
) {
    if ctx.is_redundant_semicolon(&child) {
        return;
    }
    if prev_child.is_some_and(|p| ctx.has_user_newline_between(&p, &child)) {
        items.push(BlockItem::new(Doc::text(";")));
    } else if let Some(last) = items.last_mut() {
        last.doc = Doc::concat(vec![last.doc.clone(), Doc::text(";")]);
    } else {
        items.push(BlockItem::new(Doc::text(";")));
    }
}

fn handle_class_body_accessor<'a>(
    child: Node<'a>,
    items: &mut Vec<BlockItem<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let doc = Doc::indent(Doc::concat(vec![Doc::HardLine, lower_fn(child, ctx)]));
    if let Some(last) = items.last_mut() {
        last.doc = Doc::concat(vec![last.doc.clone(), doc]);
    } else {
        items.push(BlockItem::new(doc));
    }
}

fn push_class_body_member<'a>(
    child: Node<'a>,
    prev_child: Option<Node<'a>>,
    brace_node: Option<Node<'a>>,
    last_enum_entry: Option<Node<'a>>,
    has_non_redundant_semicolon: bool,
    items: &mut Vec<BlockItem<'a>>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) {
    let mut entry_doc = lower_fn(child, ctx);
    if child.kind() == "enum_entry"
        && Some(child) == last_enum_entry
        && !has_non_redundant_semicolon
        && ctx.options.trailing_comma_strategy != TrailingCommaStrategy::None
        && !child.next_sibling().is_some_and(|s| s.kind() == ",")
    {
        entry_doc = Doc::concat(vec![entry_doc, Doc::text(",")]);
    }

    let blank = if let Some(p) = prev_child {
        ctx.has_blank_line_between(&p, &child) || requires_blank_line_separation(&p, &child)
    } else if let Some(b) = brace_node {
        ctx.has_blank_line_between(&b, &child)
    } else {
        false
    };
    items.push(BlockItem::new(entry_doc).with_blank_line_before(blank));
}

fn handle_class_body_comma<'a>(
    child: Node<'a>,
    items: &mut [BlockItem<'a>],
    ctx: &LowerContext<'a, '_>,
) {
    if child.next_sibling().is_some_and(|next| {
        next.kind() == ";"
            && !ctx.is_redundant_semicolon(&next)
            && next.start_position().row == child.start_position().row
    }) {
        return;
    }
    if let Some(last) = items.last_mut() {
        last.doc = Doc::concat(vec![last.doc.clone(), Doc::text(",")]);
    }
}

fn assemble_class_body_result<'a>(
    node: &Node<'a>,
    items: Vec<BlockItem<'a>>,
    opening_comment: Option<Doc<'a>>,
    prev_child: Option<Node<'a>>,
    ctx: &LowerContext<'a, '_>,
) -> Doc<'a> {
    let close_brace = node.children(&mut node.walk()).find(|c| c.kind() == "}");
    let blank_at_end = close_brace
        .is_some_and(|cb| prev_child.is_some_and(|p| ctx.has_blank_line_between(&p, &cb)));
    format_braced_block_ext(items, Vec::new(), opening_comment, blank_at_end)
}

fn find_enum_meta<'a>(node: &Node<'a>, ctx: &LowerContext<'a, '_>) -> (bool, Option<Node<'a>>) {
    let has_non_redundant_semicolon = node
        .children(&mut node.walk())
        .any(|c| c.kind() == ";" && !ctx.is_redundant_semicolon(&c));
    let last_enum_entry = node
        .children(&mut node.walk())
        .filter(|c| c.kind() == "enum_entry")
        .last();
    (has_non_redundant_semicolon, last_enum_entry)
}

pub fn lower_class_body<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut items: Vec<BlockItem<'a>> = Vec::new();
    let mut prev_child = None;
    let mut opening_comment = None;
    let mut brace_node = None;

    let (has_non_redundant_semicolon, last_enum_entry) = find_enum_meta(&node, ctx);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" {
            brace_node = Some(child);
            continue;
        }
        if kind == "}" {
            continue;
        }

        if matches!(kind, "comment" | "line_comment" | "multiline_comment")
            && handle_class_body_comment(
                child,
                prev_child,
                brace_node,
                &mut items,
                &mut opening_comment,
                ctx,
                lower_fn,
            )
        {
            prev_child = Some(child);
            continue;
        }

        if kind == "," {
            handle_class_body_comma(child, &mut items, ctx);
            continue;
        }

        if kind == ";" {
            handle_class_body_semicolon(child, prev_child, &mut items, ctx);
            if !ctx.is_redundant_semicolon(&child) {
                prev_child = Some(child);
            }
            continue;
        }

        let is_accessor_or_backing = matches!(kind, "getter" | "setter" | "backing_field")
            || (prev_child.is_some_and(|p| p.kind() == "property_declaration")
                && kind == "assignment"
                && child.child(0).is_some_and(|c| {
                    let t = ctx.node_text(&c);
                    t == "field" || t == "get()" || t.starts_with("set(")
                }));

        if is_accessor_or_backing {
            handle_class_body_accessor(child, &mut items, ctx, lower_fn);
            prev_child = Some(child);
            continue;
        }

        push_class_body_member(
            child,
            prev_child,
            brace_node,
            last_enum_entry,
            has_non_redundant_semicolon,
            &mut items,
            ctx,
            lower_fn,
        );
        prev_child = Some(child);
    }

    assemble_class_body_result(&node, items, opening_comment, prev_child, ctx)
}

pub fn lower_enum_entry<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body" {
            docs.push(Doc::space());
        }
        docs.push(lower_fn(child, ctx));
    }
    Doc::concat(docs)
}

pub fn lower_anonymous_initializer<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    Doc::concat(vec![
        Doc::text("init "),
        super::block::lower_block_node(node, ctx, lower_fn),
    ])
}
