use tree_sitter::Node;

use crate::doc::{Doc, GroupId};
use crate::lower::builders::{ListItem, format_delimited_list};
use crate::lower::context::LowerContext;

pub fn lower_multi_variable_declaration_with_gid<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> (Doc<'a>, Option<GroupId>) {
    let mut items = Vec::new();
    let mut open_delim = "(";
    let mut close_delim = ")";
    let mut has_trailing_comma = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "[" => (open_delim, close_delim) = ("[", "]"),
            "(" | ")" | "]" => {}
            "," => has_trailing_comma = true,
            _ => {
                has_trailing_comma = false;
                items.push(ListItem::new(lower_fn(child, ctx)));
            }
        }
    }

    let (strategy, omit_closing) = if ctx.style.preset == crate::options::StylePreset::Kotlinlang
        && !ctx.options.use_experimental_engine
    {
        (crate::options::TrailingCommaStrategy::None, true)
    } else {
        (ctx.options.trailing_comma_strategy, false)
    };

    let list_doc = format_delimited_list(
        open_delim,
        close_delim,
        items,
        Vec::new(),
        has_trailing_comma,
        strategy,
        true,
        omit_closing,
        false,
    );

    let gid = if omit_closing {
        Some(ctx.next_group_id())
    } else {
        None
    };
    let doc = if let Some(id) = gid {
        Doc::group_with_id(id, list_doc)
    } else {
        Doc::group(list_doc)
    };
    (doc, gid)
}

pub fn lower_multi_variable_declaration<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    lower_fn: &dyn Fn(Node<'a>, &LowerContext<'a, '_>) -> Doc<'a>,
) -> Doc<'a> {
    lower_multi_variable_declaration_with_gid(node, ctx, lower_fn).0
}
