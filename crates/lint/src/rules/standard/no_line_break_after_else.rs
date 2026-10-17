//! Built-in AST rule forbidding line breaks after 'else' (Ktlint rule 58).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that forbids line breaks immediately after 'else'.
pub struct NoLineBreakAfterElse;

impl AstRule for NoLineBreakAfterElse {
    fn id(&self) -> &'static str {
        "no-line-break-after-else"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["if_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut found_else = false;
        let mut else_token = None;
        let mut next_token = None;

        for child in node.children(&mut cursor) {
            if !found_else {
                if child.kind() == "else" {
                    found_else = true;
                    else_token = Some(child);
                }
                continue;
            }

            if matches!(
                child.kind(),
                "line_comment" | "multiline_comment" | "comment"
            ) {
                continue;
            }

            next_token = Some(child);
            break;
        }

        let (Some(else_node), Some(next_node)) = (else_token, next_token) else {
            return;
        };

        if next_node.start_position().row > else_node.end_position().row {
            ctx.report(else_node, "Unexpected line break after 'else'");
        }
    }
}
