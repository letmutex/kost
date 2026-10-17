//! Built-in AST rule enforcing parameter list wrapping (Ktlint rule 60).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces parameter list wrapping when parameters span multiple lines.
pub struct ParameterListWrapping;

impl AstRule for ParameterListWrapping {
    fn id(&self) -> &'static str {
        "parameter-list-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_value_parameters", "class_parameters"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let mut cursor = node.walk();
        let mut prev_delimiter: Option<tree_sitter::Node> = None;
        let mut prev_non_comment: Option<tree_sitter::Node> = None;
        let mut has_parameters = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "(" => {
                    prev_delimiter = Some(child);
                    prev_non_comment = Some(child);
                }
                "parameter" | "class_parameter" => {
                    has_parameters = true;
                    if let Some(delimiter) = prev_delimiter
                        && child.start_position().row <= delimiter.end_position().row
                    {
                        ctx.report(child, "Parameter should start on a newline");
                    }
                    prev_delimiter = None;
                    prev_non_comment = Some(child);
                }
                "," => {
                    prev_delimiter = Some(child);
                    prev_non_comment = Some(child);
                }
                ")" => {
                    if has_parameters
                        && let Some(prev) = prev_non_comment
                        && child.start_position().row <= prev.end_position().row
                    {
                        ctx.report(child, "Expected newline before ')'");
                    }
                    break;
                }
                kind if !matches!(kind, "line_comment" | "multiline_comment" | "comment") => {
                    prev_non_comment = Some(child);
                }
                _ => {}
            }
        }
    }
}
