//! Built-in AST rule forbidding blank lines in lists (Ktlint rule 68).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that forbids blank lines inside parameter, argument, and type lists.
pub struct NoBlankLineInList;

impl AstRule for NoBlankLineInList {
    fn id(&self) -> &'static str {
        "no-blank-line-in-list"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "function_value_parameters",
            "class_parameters",
            "primary_constructor",
            "value_arguments",
            "type_parameters",
            "type_arguments",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let mut cursor = node.walk();
        let mut last_end_row: Option<usize> = None;
        let mut close_delimiter: Option<tree_sitter::Node> = None;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "(" | "<" => {
                    if last_end_row.is_none() {
                        last_end_row = Some(child.end_position().row);
                    }
                }
                ")" | ">" => {
                    close_delimiter = Some(child);
                }
                _ => {
                    if let Some(prev_row) = last_end_row {
                        if child.start_position().row > prev_row + 1 {
                            ctx.report(child, "Unexpected blank line(s)");
                        }
                        last_end_row = Some(prev_row.max(child.end_position().row));
                    }
                }
            }
        }

        if let (Some(prev_row), Some(close)) = (last_end_row, close_delimiter)
            && close.start_position().row > prev_row + 1
        {
            ctx.report(close, "Unexpected blank line(s)");
        }
    }
}
