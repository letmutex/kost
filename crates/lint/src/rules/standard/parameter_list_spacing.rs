//! Built-in AST rule enforcing spacing inside parameter lists (Ktlint rule 35).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces zero spaces directly inside parameter list parentheses.
pub struct ParameterListSpacing;

impl AstRule for ParameterListSpacing {
    fn id(&self) -> &'static str {
        "parameter-list-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_value_parameters", "primary_constructor"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut left_paren = None;
        let mut right_paren = None;
        let mut parameter_count = 0;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "(" => left_paren = Some(child),
                ")" => right_paren = Some(child),
                "parameter" | "class_parameter" => parameter_count += 1,
                _ => {}
            }
        }

        let (Some(lpar), Some(rpar)) = (left_paren, right_paren) else {
            return;
        };

        match spaces_after(lpar, source) {
            WhitespaceAfter::Spaces(count) if count > 0 => {
                ctx.report(lpar, "Unexpected spacing after '('");
            }
            WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
                ctx.report(lpar, "Unexpected spacing after '('");
            }
            _ => {}
        }

        if parameter_count > 0 {
            match spaces_before(rpar, source) {
                WhitespaceBefore::Spaces(count) if count > 0 => {
                    ctx.report(rpar, "Unexpected spacing before ')'");
                }
                _ => {}
            }
        }
    }
}
