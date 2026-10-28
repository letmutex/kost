//! Built-in AST rule forbidding comments in type parameter lists (Ktlint rule 99).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::node_kind;

/// Rule that disallows misplaced comments inside generic type parameter lists.
pub struct TypeParameterComment;

impl AstRule for TypeParameterComment {
    fn id(&self) -> &'static str {
        "type-parameter-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(parent) = ctx.parent() else {
            return;
        };
        let parent_kind = node_kind(parent);
        if parent_kind == "type_parameter" {
            ctx.report(
                node,
                "A (block or EOL) comment inside or on same line after a 'type_parameter' is not allowed. It may be placed on a separate line above.",
            );
        } else if parent_kind == "type_parameters"
            && let Some(prev) = ctx.prev_sibling()
            && node_kind(prev) != ">"
            && node.start_position().row == prev.end_position().row
        {
            ctx.report(
                node,
                "A comment in a 'type_parameter_list' is only allowed when placed on a separate line",
            );
        }
    }
}
