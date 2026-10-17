//! Built-in AST rule enforcing single space after the `fun` keyword (Ktlint rule 27).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces exactly one space following the `fun` keyword.
pub struct FunKeywordSpacing;

impl AstRule for FunKeywordSpacing {
    fn id(&self) -> &'static str {
        "fun-keyword-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "fun" {
                match spaces_after(child, source) {
                    WhitespaceAfter::Spaces(0) => {
                        ctx.report(child, "Missing spacing after 'fun'");
                    }
                    WhitespaceAfter::Spaces(count) if count > 1 => {
                        ctx.report(child, "Unexpected spacing after 'fun'");
                    }
                    WhitespaceAfter::EndOfLine { .. } => {
                        ctx.report(child, "Unexpected spacing after 'fun'");
                    }
                    _ => {}
                }
                break;
            }
        }
    }
}
