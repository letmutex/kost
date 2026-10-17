//! Built-in AST rule forbidding comments in value argument lists (Ktlint rule 100).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that disallows misplaced comments inside call argument lists.
pub struct ValueArgumentComment;

impl AstRule for ValueArgumentComment {
    fn id(&self) -> &'static str {
        "value-argument-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["value_arguments", "value_argument"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "value_argument" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if is_comment(child.kind()) {
                    ctx.report(
                        child,
                        "A (block or EOL) comment inside or on same line after a 'value_argument' is not allowed. It may be placed on a separate line above.",
                    );
                }
            }
        } else if node.kind() == "value_arguments" {
            let mut cursor = node.walk();
            let mut prev_arg: Option<tree_sitter::Node> = None;
            for child in node.children(&mut cursor) {
                if is_comment(child.kind()) {
                    if let Some(prev) = prev_arg
                        && child.start_position().row == prev.end_position().row
                    {
                        ctx.report(
                            child,
                            "A (block or EOL) comment inside or on same line after a 'value_argument' is not allowed. It may be placed on a separate line above.",
                        );
                    }
                } else if child.kind() == "value_argument" {
                    prev_arg = Some(child);
                }
            }
        }
    }
}

fn is_comment(kind: &str) -> bool {
    matches!(kind, "line_comment" | "multiline_comment" | "comment")
}
