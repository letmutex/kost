//! Built-in AST rule forbidding line breaks before '=' in assignments and property declarations (Ktlint rule 59).

use crate::context::LintCtx;
use crate::rule::AstRule;

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
        for child in node.children(&mut cursor) {
            if child.kind() == "=" {
                if let Some(prev) = prev_non_comment_sibling(child)
                    && prev.end_position().row < child.start_position().row
                {
                    ctx.report(child, "Line break is not allowed before '='");
                }
                break;
            }
        }
    }
}

fn prev_non_comment_sibling(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        if !matches!(
            sibling.kind(),
            "line_comment" | "multiline_comment" | "comment"
        ) {
            return Some(sibling);
        }
        current = sibling.prev_sibling();
    }
    None
}
