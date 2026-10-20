use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::callable_reference::lower_double_colon_navigation;
pub use crate::lower::chain::{
    count_chain_lambdas, has_call_in_receiver_chain, is_call_receiver, is_call_receiver_in_chain,
    is_chain_broken, is_navigation_chain_root, is_part_of_call_chain, is_spine_root,
    is_super_or_this, is_type_name_receiver,
};
use crate::lower::context::LowerContext;
use crate::lower::lower_node;

pub fn lower_navigation_expression<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut operand = None;
    let mut comments = Vec::new();
    let mut suffix = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if operand.is_none() {
            operand = Some(child);
        } else if matches!(
            child.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            comments.push(child);
        } else {
            suffix = Some(child);
        }
    }

    let Some(op_node) = operand else {
        return Doc::empty();
    };

    let op_doc = lower_node(op_node, ctx);

    if let Some(suf_node) = suffix {
        let has_double_colon = suf_node.child(0).is_some_and(|c| c.kind() == "::");
        if has_double_colon {
            return lower_double_colon_navigation(&node, op_doc, suf_node, ctx);
        }

        let is_breakable = !comments.is_empty()
            || (!is_super_or_this(&op_node)
                && (is_call_receiver(&op_node)
                    || (ctx.options.block_indent == 4 && has_call_in_receiver_chain(&op_node))
                    || (ctx.options.block_indent != 4
                        && ctx.has_newline_between(&op_node, &suf_node))));
        // In Google/ktfmt style (block_indent != 4), calls following a multiline raw string
        // (such as .trimIndent() or .trimMargin()) must wrap to a new line.
        // In Kotlinlang/ktlint style (block_indent == 4), they attach directly to the closing quotes.
        let is_raw_string = ctx.options.block_indent != 4
            && (matches!(op_node.kind(), "multiline_string_literal")
                || (op_node.kind() == "string_literal"
                    && op_node.start_position().row != op_node.end_position().row));
        let suf_doc =
            lower_navigation_suffix_with_break(suf_node, ctx, is_breakable, is_raw_string);
        let mut parts = Vec::new();
        let mut prev = op_node;
        for c in comments {
            if ctx.has_newline_between(&prev, &c) {
                parts.push(Doc::HardLine);
            } else {
                parts.push(Doc::space());
            }
            parts.push(lower_node(c, ctx));
            prev = c;
        }
        parts.push(suf_doc);
        let has_hardline = op_doc.has_hardline();
        let chain_doc = if is_breakable || is_raw_string {
            Doc::Concat(vec![op_doc, Doc::continuation_indent(Doc::Concat(parts))])
        } else {
            Doc::Concat(vec![op_doc, Doc::Concat(parts)])
        };
        if is_navigation_chain_root(&node) {
            let chain_len = crate::lower::expression_body::collapsed_text_len(ctx.node_text(&node));
            let is_broken = is_chain_broken(&node, ctx, chain_len) || has_hardline;
            if is_broken && is_breakable {
                chain_doc
            } else {
                Doc::Group(Box::new(chain_doc))
            }
        } else {
            chain_doc
        }
    } else {
        op_doc
    }
}

pub fn lower_navigation_suffix_with_break<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    is_breakable: bool,
    force_hardline: bool,
) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "." || kind == "?." {
            if force_hardline {
                docs.push(Doc::HardLine);
            } else if is_breakable {
                docs.push(Doc::SoftLine);
            }
            docs.push(Doc::text(ctx.node_text(&child)));
        } else {
            docs.push(lower_node(child, ctx));
        }
    }
    Doc::Concat(docs)
}

pub fn lower_navigation_suffix<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    lower_navigation_suffix_with_break(node, ctx, true, false)
}
