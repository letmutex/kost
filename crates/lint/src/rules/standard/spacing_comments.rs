//! Built-in AST rule enforcing a blank line before declarations preceded by comments (Ktlint rule 11).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{prev_declaration_sibling, prev_meaningful_sibling};

/// Rule that enforces a blank line before a declaration preceded by a comment.
pub struct SpacingBetweenDeclarationsWithComments;

impl AstRule for SpacingBetweenDeclarationsWithComments {
    fn id(&self) -> &'static str {
        "spacing-between-declarations-with-comments"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "class_declaration",
            "object_declaration",
            "property_declaration",
            "secondary_constructor",
            "anonymous_initializer",
            "type_alias",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node
            .parent()
            .map(|p| p.kind() == "statements")
            .unwrap_or(false)
        {
            return;
        }

        let Some(prev_decl) = prev_declaration_sibling(node) else {
            return;
        };

        let Some(meaningful_sibling) = prev_meaningful_sibling(node) else {
            return;
        };

        if !matches!(
            meaningful_sibling.kind(),
            "line_comment" | "multiline_comment"
        ) {
            return;
        }

        let prev_end_row = prev_decl.end_position().row;
        let mut first_comment = meaningful_sibling;
        let mut current = meaningful_sibling.prev_sibling();
        while let Some(sibling) = current {
            if sibling == prev_decl || sibling.start_position().row <= prev_end_row {
                break;
            }
            if matches!(sibling.kind(), "line_comment" | "multiline_comment") {
                first_comment = sibling;
            } else if sibling.is_named() {
                break;
            }
            current = sibling.prev_sibling();
        }

        let comment_start_row = first_comment.start_position().row;

        if comment_start_row == prev_end_row || node.start_position().row == comment_start_row {
            return;
        }

        if comment_start_row <= prev_end_row + 1 {
            ctx.report(first_comment, "Expected a blank line before comment");
        }
    }
}
