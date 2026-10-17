//! Built-in AST rule enforcing curly braces on multiline loop statements (Ktlint rule 57).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_braced_body;

/// Rule that enforces curly braces on loop bodies that span multiple lines.
pub struct MultilineLoop;

impl AstRule for MultilineLoop {
    fn id(&self) -> &'static str {
        "multiline-loop"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["for_statement", "while_statement", "do_while_statement"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "control_structure_body" {
                let is_multiline = node.start_position().row != child.end_position().row;
                if is_multiline && !is_braced_body(child) {
                    ctx.report(child, "Missing { ... }");
                }
                break;
            }
        }
    }
}
