//! Built-in AST rule enforcing chain operator wrapping (Ktlint rule 46).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces chain operators (., ?., ?:) are placed at the start of continuation lines.
pub struct ChainWrapping;

impl AstRule for ChainWrapping {
    fn id(&self) -> &'static str {
        "chain-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[".", "?.", "?:"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(parent) = ctx.parent() else {
            return;
        };

        let is_valid_chain_parent = match node.kind() {
            "." | "?." => matches!(parent.kind(), "navigation_expression" | "navigation_suffix"),
            "?:" => matches!(parent.kind(), "elvis_expression" | "binary_expression"),
            _ => false,
        };

        if !is_valid_chain_parent {
            return;
        }

        let is_trailing = matches!(
            spaces_after(node, source),
            WhitespaceAfter::EndOfLine { .. }
        ) || is_followed_by_newline_and_operand(node, parent);

        if is_trailing {
            let operator_text = node.utf8_text(source).unwrap_or(node.kind());
            let message = crate::concat_str!("Line must not end with \"", operator_text, "\"");
            ctx.report(node, message);
        }
    }
}

fn is_followed_by_newline_and_operand(node: tree_sitter::Node, parent: tree_sitter::Node) -> bool {
    let mut cursor = parent.walk();
    let mut found_node = false;
    for child in parent.children(&mut cursor) {
        if !found_node {
            if child == node {
                found_node = true;
            }
            continue;
        }
        if matches!(
            child.kind(),
            "line_comment" | "multiline_comment" | "comment"
        ) {
            continue;
        }
        return child.start_position().row > node.start_position().row;
    }
    false
}
