//! Built-in AST rule enforcing single space after function type modifier (Ktlint rule 30).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces single space between a function type modifier (e.g. `suspend`) and `() -> ...`.
pub struct FunctionTypeModifierSpacing;

impl AstRule for FunctionTypeModifierSpacing {
    fn id(&self) -> &'static str {
        "function-type-modifier-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_type"]
    }

    fn check_node(&self, _node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(prev) = ctx.prev_sibling() else {
            return;
        };

        if prev.kind() != "type_modifiers" {
            return;
        }

        let child_count = prev.child_count();
        if child_count == 0 {
            return;
        }

        let Some(modifier) = prev.child(child_count - 1) else {
            return;
        };

        if modifier.kind() != "suspend" {
            return;
        }

        let mod_str = modifier.kind();
        match spaces_after(modifier, source) {
            WhitespaceAfter::Spaces(0) => {
                let message = common::concat_str!("Missing spacing after '", mod_str, "'");
                ctx.report(modifier, message);
            }
            WhitespaceAfter::Spaces(count) if count > 1 => {
                let message = common::concat_str!("Unexpected spacing after '", mod_str, "'");
                ctx.report(modifier, message);
            }
            WhitespaceAfter::EndOfLine { .. } => {
                let message = common::concat_str!("Unexpected spacing after '", mod_str, "'");
                ctx.report(modifier, message);
            }
            _ => {}
        }
    }
}
