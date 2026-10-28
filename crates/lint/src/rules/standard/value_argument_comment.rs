//! Built-in AST rule forbidding comments in value argument lists (Ktlint rule 100).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::node_kind;

/// Rule that disallows misplaced comments inside call argument lists.
pub struct ValueArgumentComment;

impl AstRule for ValueArgumentComment {
    fn id(&self) -> &'static str {
        "value-argument-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(parent) = ctx.parent() else {
            return;
        };
        let parent_kind = node_kind(parent);
        if parent_kind == "value_argument" {
            ctx.report(
                node,
                "A (block or EOL) comment inside or on same line after a 'value_argument' is not allowed. It may be placed on a separate line above.",
            );
        } else if parent_kind == "value_arguments" {
            let mut cursor = parent.walk();
            let mut prev_arg: Option<tree_sitter::Node> = None;
            for child in parent.children(&mut cursor) {
                if child == node {
                    if let Some(prev) = prev_arg
                        && node.start_position().row == prev.end_position().row
                    {
                        ctx.report(
                            node,
                            "A (block or EOL) comment inside or on same line after a 'value_argument' is not allowed. It may be placed on a separate line above.",
                        );
                    }
                    break;
                } else if node_kind(child) == "value_argument" {
                    prev_arg = Some(child);
                }
            }
        }
    }
}
