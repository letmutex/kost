//! Built-in AST rule enforcing single space after return type colon (Ktlint rule 28).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces exactly one space between the colon and the return type.
pub struct FunctionReturnTypeSpacing;

impl AstRule for FunctionReturnTypeSpacing {
    fn id(&self) -> &'static str {
        "function-return-type-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut seen_params = false;

        for child in node.children(&mut cursor) {
            if child.kind() == "function_value_parameters" {
                seen_params = true;
            } else if seen_params && child.kind() == ":" {
                match spaces_after(child, source) {
                    WhitespaceAfter::Spaces(0) => {
                        ctx.report(child, "Missing spacing after ':'");
                    }
                    WhitespaceAfter::Spaces(count) if count > 1 => {
                        ctx.report(child, "Unexpected spacing after ':'");
                    }
                    WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
                        ctx.report(child, "Unexpected spacing after ':'");
                    }
                    _ => {}
                }
                break;
            }
        }
    }
}
