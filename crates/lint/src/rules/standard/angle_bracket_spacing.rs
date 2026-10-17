//! Built-in AST rule enforcing spacing around angle brackets (Ktlint rule 25).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around angle brackets in type arguments and type parameters.
pub struct AngleBracketSpacing;

impl AstRule for AngleBracketSpacing {
    fn id(&self) -> &'static str {
        "spacing-around-angle-brackets"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["type_arguments", "type_parameters"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let child_count = node.child_count();
        if child_count < 2 {
            return;
        }

        let Some(left_angle) = node.child(0) else {
            return;
        };
        let Some(right_angle) = node.child(child_count - 1) else {
            return;
        };

        if left_angle.kind() != "<" || right_angle.kind() != ">" {
            return;
        }

        // Check spacing before '<' when attached to type or class name
        let should_check_before = node.kind() == "type_arguments"
            || node
                .prev_sibling()
                .is_some_and(|prev| matches!(prev.kind(), "type_identifier" | "simple_identifier"));

        if should_check_before {
            match spaces_before(left_angle, source) {
                WhitespaceBefore::Spaces(count) if count > 0 => {
                    ctx.report(left_angle, "Unexpected spacing before '<'");
                }
                WhitespaceBefore::StartOfLine => {
                    ctx.report(left_angle, "Unexpected spacing before '<'");
                }
                _ => {}
            }
        }

        // Check spacing after '<'
        match spaces_after(left_angle, source) {
            WhitespaceAfter::Spaces(count) if count > 0 => {
                ctx.report(left_angle, "Unexpected spacing after '<'");
            }
            WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
                ctx.report(left_angle, "Unexpected spacing after '<'");
            }
            _ => {}
        }

        // Check spacing before '>'
        match spaces_before(right_angle, source) {
            WhitespaceBefore::Spaces(count) if count > 0 => {
                ctx.report(right_angle, "Unexpected spacing before '>'");
            }
            _ => {}
        }
    }
}
