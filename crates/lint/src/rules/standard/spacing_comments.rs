//! Built-in AST rule enforcing a blank line before declarations preceded by comments (Ktlint rule 11).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::node_kind;

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
        let Some(parent) = ctx.parent() else {
            return;
        };
        if node_kind(parent) == "statements" {
            return;
        }

        let Some(meaningful_sibling) = ctx.prev_meaningful_sibling() else {
            return;
        };

        if !matches!(
            node_kind(meaningful_sibling),
            "line_comment" | "multiline_comment"
        ) {
            return;
        }

        let Some(prev_decl) = ctx.prev_declaration_sibling() else {
            return;
        };

        let prev_end_row = prev_decl.end_position().row;
        let first_comment = find_first_leading_comment(parent, prev_decl, node, meaningful_sibling);
        let comment_start_row = first_comment.start_position().row;

        if comment_start_row == prev_end_row || node.start_position().row == comment_start_row {
            return;
        }

        if comment_start_row <= prev_end_row + 1 {
            ctx.report(first_comment, "Expected a blank line before comment");
        }
    }
}

fn find_first_leading_comment<'a>(
    parent: tree_sitter::Node<'a>,
    prev_decl: tree_sitter::Node<'a>,
    curr_decl: tree_sitter::Node<'a>,
    meaningful_sibling: tree_sitter::Node<'a>,
) -> tree_sitter::Node<'a> {
    let prev_end_row = prev_decl.end_position().row;
    let prev_end_byte = prev_decl.end_byte();
    let curr_start_byte = curr_decl.start_byte();
    let mut candidate: Option<tree_sitter::Node<'a>> = None;
    let mut cursor = parent.walk();
    for sibling in parent.children(&mut cursor) {
        if sibling.start_byte() < prev_end_byte || sibling == prev_decl {
            continue;
        }
        if sibling.start_byte() >= curr_start_byte || sibling == curr_decl {
            break;
        }
        if sibling.start_position().row <= prev_end_row {
            candidate = None;
            continue;
        }
        if matches!(node_kind(sibling), "line_comment" | "multiline_comment") {
            if candidate.is_none() {
                candidate = Some(sibling);
            }
        } else if sibling.is_named() {
            candidate = None;
        }
    }
    candidate.unwrap_or(meaningful_sibling)
}
