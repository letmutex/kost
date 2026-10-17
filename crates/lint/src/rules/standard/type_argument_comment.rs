//! Built-in AST rule forbidding comments in type argument lists (Ktlint rule 98).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that disallows misplaced comments inside type argument lists.
pub struct TypeArgumentComment;

impl AstRule for TypeArgumentComment {
    fn id(&self) -> &'static str {
        "type-argument-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["type_arguments", "type_projection"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "type_projection" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if is_comment(child.kind()) {
                    ctx.report(
                        child,
                        "A (block or EOL) comment inside or on same line after a 'type_projection' is not allowed. It may be placed on a separate line above.",
                    );
                }
            }
        } else if node.kind() == "type_arguments" {
            let mut cursor = node.walk();
            let mut prev_node: Option<tree_sitter::Node> = None;
            for child in node.children(&mut cursor) {
                let kind = child.kind();
                if is_comment(kind) {
                    if let Some(prev) = prev_node
                        && child.start_position().row == prev.end_position().row
                    {
                        ctx.report(
                            child,
                            "A comment in a 'type_argument_list' is only allowed when placed on a separate line",
                        );
                    }
                    prev_node = Some(child);
                } else if kind != ">" {
                    prev_node = Some(child);
                }
            }
        }
    }
}

fn is_comment(kind: &str) -> bool {
    matches!(kind, "line_comment" | "multiline_comment" | "comment")
}
