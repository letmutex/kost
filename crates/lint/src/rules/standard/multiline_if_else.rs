//! Built-in AST rule enforcing curly braces on multiline if-else statements (Ktlint rule 56).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_braced_body;

/// Rule that enforces curly braces on both branches of an if-else statement if either branch is multiline.
pub struct MultilineIfElse;

impl AstRule for MultilineIfElse {
    fn id(&self) -> &'static str {
        "multiline-if-else"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["if_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut consequence = None;
        let mut alternative = None;
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "control_structure_body" {
                if consequence.is_none() {
                    consequence = Some(child);
                } else if alternative.is_none() {
                    alternative = Some(child);
                }
            }
        }

        let (Some(cons_node), Some(alt_node)) = (consequence, alternative) else {
            return;
        };

        let is_multiline = node.start_position().row != cons_node.end_position().row
            || alt_node.start_position().row != alt_node.end_position().row;

        if !is_multiline {
            return;
        }

        if !is_braced_body(cons_node) {
            ctx.report(cons_node, "Missing { ... }");
        }

        if !is_else_if(alt_node) && !is_braced_body(alt_node) {
            ctx.report(alt_node, "Missing { ... }");
        }
    }
}

fn is_else_if(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "if_expression" {
            return true;
        }
    }
    false
}
