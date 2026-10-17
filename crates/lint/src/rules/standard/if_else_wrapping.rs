//! Built-in AST rule enforcing if-else wrapping (Ktlint rule 54).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces 'else' follows the closing brace of 'if' on the same line.
pub struct IfElseWrapping;

impl AstRule for IfElseWrapping {
    fn id(&self) -> &'static str {
        "if-else-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["if_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut closing_brace = None;
        let mut else_token = None;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "control_structure_body" && closing_brace.is_none() {
                closing_brace = find_closing_brace(child);
            } else if child.kind() == "else" {
                else_token = Some(child);
                break;
            }
        }

        let (Some(rbrace), Some(else_node)) = (closing_brace, else_token) else {
            return;
        };

        if else_node.start_position().row > rbrace.start_position().row {
            ctx.report(
                else_node,
                "Expected 'else' on the same line as the closing brace",
            );
        }
    }
}

fn find_closing_brace(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if node.kind() == "control_structure_body" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "}" {
                return Some(child);
            }
        }
    } else if node.kind() == "}" {
        return Some(node);
    }
    None
}
