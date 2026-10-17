//! Built-in AST rule enforcing spacing around colons (Ktlint rule 14).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around colons based on syntactic context.
pub struct ColonSpacing;

impl AstRule for ColonSpacing {
    fn id(&self) -> &'static str {
        "colon-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[":"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let parent_kind = ctx.parent().map(|p| p.kind()).unwrap_or("");

        if matches!(parent_kind, "use_site_target" | "file_annotation") {
            if let WhitespaceBefore::Spaces(n) = spaces_before(node, source)
                && n > 0
            {
                ctx.report(node, "Unexpected spacing before ':'");
            }
            if let WhitespaceAfter::Spaces(n) = spaces_after(node, source)
                && n > 0
            {
                ctx.report(node, "Unexpected spacing after ':'");
            }
            return;
        }

        let is_inheritance_or_constraint = matches!(
            parent_kind,
            "class_declaration"
                | "object_declaration"
                | "object_literal"
                | "companion_object"
                | "secondary_constructor"
                | "delegation_specifier"
                | "delegation_specifiers"
                | "type_constraint"
                | "constructor_delegation_call"
                | "type_parameter"
        );

        match spaces_before(node, source) {
            WhitespaceBefore::StartOfLine => {
                ctx.report(node, "Unexpected newline before ':'");
            }
            WhitespaceBefore::Spaces(0) if is_inheritance_or_constraint => {
                ctx.report(node, "Missing spacing before ':'");
            }
            WhitespaceBefore::Spaces(n) if is_inheritance_or_constraint && n > 1 => {
                ctx.report(node, "Unexpected spacing before ':'");
            }
            WhitespaceBefore::Spaces(n) if !is_inheritance_or_constraint && n > 0 => {
                ctx.report(node, "Unexpected spacing before ':'");
            }
            _ => {}
        }

        match spaces_after(node, source) {
            WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
                ctx.report(node, "Unexpected spacing after ':'");
            }
            WhitespaceAfter::Spaces(0) => {
                ctx.report(node, "Missing spacing after ':'");
            }
            WhitespaceAfter::Spaces(n) if n > 1 => {
                ctx.report(node, "Unexpected spacing after ':'");
            }
            _ => {}
        }
    }
}
