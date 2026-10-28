//! Built-in AST rule forbidding comments in value parameter lists (Ktlint rule 101).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{is_kdoc, node_kind};

/// Rule that disallows misplaced comments inside parameter lists.
pub struct ValueParameterComment;

impl AstRule for ValueParameterComment {
    fn id(&self) -> &'static str {
        "value-parameter-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(parent) = ctx.parent() else {
            return;
        };
        let parent_kind = node_kind(parent);
        if matches!(parent_kind, "parameter" | "class_parameter") {
            if ctx.prev_sibling().is_none()
                && let Ok(text) = node.utf8_text(source)
                && is_kdoc(text)
            {
                return;
            }
            ctx.report(
                node,
                "A comment inside or on same line after a 'value_parameter' is not allowed. It may be placed on a separate line above.",
            );
        } else if matches!(
            parent_kind,
            "function_value_parameters" | "class_parameters" | "primary_constructor"
        ) {
            let mut cursor = parent.walk();
            let mut prev_param: Option<tree_sitter::Node> = None;
            for child in parent.children(&mut cursor) {
                if child == node {
                    if let Some(prev) = prev_param
                        && node.start_position().row == prev.end_position().row
                    {
                        ctx.report(
                            node,
                            "A comment inside or on same line after a 'value_parameter' is not allowed. It may be placed on a separate line above.",
                        );
                    }
                    break;
                } else if matches!(node_kind(child), "parameter" | "class_parameter") {
                    prev_param = Some(child);
                }
            }
        }
    }
}
