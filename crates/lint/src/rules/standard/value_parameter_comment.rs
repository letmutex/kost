//! Built-in AST rule forbidding comments in value parameter lists (Ktlint rule 101).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_kdoc;

/// Rule that disallows misplaced comments inside parameter lists.
pub struct ValueParameterComment;

impl AstRule for ValueParameterComment {
    fn id(&self) -> &'static str {
        "value-parameter-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "function_value_parameters",
            "class_parameters",
            "primary_constructor",
            "parameter",
            "class_parameter",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if matches!(node.kind(), "parameter" | "class_parameter") {
            let mut cursor = node.walk();
            for (index, child) in node.children(&mut cursor).enumerate() {
                if is_comment(child.kind()) {
                    if index == 0
                        && let Ok(text) = child.utf8_text(source)
                        && is_kdoc(text)
                    {
                        continue;
                    }
                    ctx.report(
                        child,
                        "A comment inside or on same line after a 'value_parameter' is not allowed. It may be placed on a separate line above.",
                    );
                }
            }
        } else if matches!(
            node.kind(),
            "function_value_parameters" | "class_parameters" | "primary_constructor"
        ) {
            let mut cursor = node.walk();
            let mut prev_param: Option<tree_sitter::Node> = None;
            for child in node.children(&mut cursor) {
                if is_comment(child.kind()) {
                    if let Some(prev) = prev_param
                        && child.start_position().row == prev.end_position().row
                    {
                        ctx.report(
                            child,
                            "A comment inside or on same line after a 'value_parameter' is not allowed. It may be placed on a separate line above.",
                        );
                    }
                } else if matches!(child.kind(), "parameter" | "class_parameter") {
                    prev_param = Some(child);
                }
            }
        }
    }
}

fn is_comment(kind: &str) -> bool {
    matches!(kind, "line_comment" | "multiline_comment" | "comment")
}
