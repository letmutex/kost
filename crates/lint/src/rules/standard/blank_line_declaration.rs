//! Built-in AST rule enforcing a blank line before declarations (Ktlint rule 9).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{
    has_blank_line_between_decls, has_declaration_annotations, prev_declaration_sibling,
};

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
        if has_declaration_annotations(node) {
            return;
        }

        let Some(prev_decl) = prev_declaration_sibling(node) else {
            return;
        };

        if !has_blank_line_between_decls(prev_decl, node) {
            ctx.report(node, "Expected a blank line before declaration");
        }
    }
}
