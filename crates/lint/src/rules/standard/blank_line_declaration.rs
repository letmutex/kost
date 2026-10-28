//! Built-in AST rule enforcing a blank line before declarations (Ktlint rule 9).

use super::helpers::has_blank_line_between_decls_with_context;
use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{has_declaration_annotations, node_kind};

/// Rule that enforces a blank line before major declarations.
pub struct BlankLineBeforeDeclaration;

impl AstRule for BlankLineBeforeDeclaration {
    fn id(&self) -> &'static str {
        "blank-line-before-declaration"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "function_declaration",
            "class_declaration",
            "object_declaration",
            "secondary_constructor",
            "anonymous_initializer",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(prev_decl) = ctx.prev_declaration_sibling() else {
            return;
        };

        if has_declaration_annotations(node) {
            return;
        }

        if node.start_position().row == node.end_position().row
            && prev_decl.start_position().row == prev_decl.end_position().row
            && !has_member_body(node)
            && !has_member_body(prev_decl)
        {
            return;
        }

        if !has_blank_line_between_decls_with_context(
            prev_decl,
            node,
            ctx.prev_sibling(),
            ctx.parent(),
        ) {
            ctx.report(node, "Expected a blank line before declaration");
        }
    }
}

fn has_member_body(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            node_kind(child),
            "class_body" | "function_body" | "enum_class_body" | "block"
        ) {
            return true;
        }
    }
    false
}
