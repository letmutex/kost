//! Built-in AST rule enforcing space before then branch (Ktlint rule 36).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces a single space between condition ')' and the then branch expression.
pub struct ThenSpacing;

impl AstRule for ThenSpacing {
    fn id(&self) -> &'static str {
        "then-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["if_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut condition_rpar = None;
        let mut seen_else = false;

        for child in node.children(&mut cursor) {
            if child.kind() == ")" {
                condition_rpar = Some(child);
            } else if child.kind() == "else" {
                seen_else = true;
            } else if child.kind() == "control_structure_body" {
                if seen_else {
                    break;
                }
                if let Some(first) = child.child(0)
                    && first.kind() == "{"
                {
                    break;
                }
                if let Some(rpar) = condition_rpar
                    && rpar.end_position().row == child.start_position().row
                {
                    match spaces_after(rpar, source) {
                        WhitespaceAfter::Spaces(0) => {
                            ctx.report(rpar, "Missing spacing after ')'");
                        }
                        WhitespaceAfter::Spaces(count) if count > 1 => {
                            ctx.report(rpar, "Unexpected spacing after ')'");
                        }
                        _ => {}
                    }
                }
                break;
            }
        }
    }
}
