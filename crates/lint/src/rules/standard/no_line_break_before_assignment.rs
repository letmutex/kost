//! Built-in AST rule forbidding line breaks before '=' in assignments and property declarations (Ktlint rule 59).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::node_kind;

/// Rule that forbids placing a line break immediately before '=' in assignments and declarations.
pub struct NoLineBreakBeforeAssignment;

impl AstRule for NoLineBreakBeforeAssignment {
    fn id(&self) -> &'static str {
        "no-line-break-before-assignment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["property_declaration", "assignment"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut prev_non_comment: Option<tree_sitter::Node> = None;
        for child in node.children(&mut cursor) {
            let kind = node_kind(child);
            if kind == "=" {
                if let Some(prev) = prev_non_comment
                    && prev.end_position().row < child.start_position().row
                {
                    ctx.report(child, "Line break is not allowed before '='");
                }
                break;
            }
            if !matches!(kind, "line_comment" | "multiline_comment" | "comment") {
                prev_non_comment = Some(child);
            }
        }
    }
}
