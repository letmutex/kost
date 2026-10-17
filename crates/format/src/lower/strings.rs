use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::context::LowerContext;
use crate::rules::multiline_string::format_multiline_raw_string;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrimKind {
    Indent,
    Margin,
}

fn detect_multiline_trim_call(string_node: Node<'_>, source: &str) -> Option<TrimKind> {
    let parent = string_node.parent()?;
    let nav_node = if parent.kind() == "navigation_expression" {
        parent
    } else {
        return None;
    };

    let mut cursor = nav_node.walk();
    for child in nav_node.children(&mut cursor) {
        if child.kind() == "navigation_suffix" {
            let mut suf_cursor = child.walk();
            for suf_child in child.children(&mut suf_cursor) {
                if suf_child.kind() == "simple_identifier"
                    && let Some(name) = source.get(suf_child.start_byte()..suf_child.end_byte())
                {
                    if name == "trimIndent" {
                        return Some(TrimKind::Indent);
                    } else if name == "trimMargin" {
                        return Some(TrimKind::Margin);
                    }
                }
            }
        }
    }
    None
}

pub fn lower_string_literal<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let raw = ctx.node_text(&node);
    if (raw.starts_with("\"\"\"") || raw.starts_with("$$\"\"\""))
        && let Some(trim_kind) = detect_multiline_trim_call(node, ctx.source)
    {
        let is_trim_margin = trim_kind == TrimKind::Margin;
        if let Some(doc) = format_multiline_raw_string(raw, is_trim_margin) {
            return doc;
        }
    }
    Doc::text(raw)
}
