//! Built-in AST rule enforcing blank lines before annotated declarations (Ktlint rule 10).

use super::helpers::has_blank_line_between_decls_with_context;
use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{has_declaration_annotations, node_kind};

/// Rule that enforces a blank line before declarations with annotations.
pub struct SpacingBetweenDeclarationsWithAnnotations;

impl AstRule for SpacingBetweenDeclarationsWithAnnotations {
    fn id(&self) -> &'static str {
        "spacing-between-declarations-with-annotations"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "property_declaration",
            "function_declaration",
            "class_declaration",
            "object_declaration",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(prev_decl) = ctx.prev_declaration_sibling() else {
            return;
        };

        let curr_annotated = has_declaration_annotations(node);
        let prev_annotated = if curr_annotated || node_kind(node) == "property_declaration" {
            has_declaration_annotations(prev_decl)
        } else {
            false
        };

        if !curr_annotated && (!prev_annotated || node_kind(node) != "property_declaration") {
            return;
        }

        if !has_blank_line_between_decls_with_context(
            prev_decl,
            node,
            ctx.prev_sibling(),
            ctx.parent(),
        ) {
            let message = if curr_annotated {
                "Expected a blank line before declaration with annotations"
            } else {
                "Expected a blank line after declaration with annotations"
            };
            ctx.report(node, message);
        }
    }
}
