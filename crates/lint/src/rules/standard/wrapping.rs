//! Built-in AST rule enforcing delimiter wrapping (Ktlint rule 64).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Fallback rule enforcing consistent bracket and delimiter positioning.
pub struct Wrapping;

impl AstRule for Wrapping {
    fn id(&self) -> &'static str {
        "wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "parenthesized_expression",
            "type_arguments",
            "type_parameters",
            "collection_literal",
            "indexing_suffix",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let mut cursor = node.walk();
        let mut open_delimiter: Option<tree_sitter::Node> = None;
        let mut close_delimiter: Option<tree_sitter::Node> = None;
        let mut first_content: Option<tree_sitter::Node> = None;
        let mut last_content: Option<tree_sitter::Node> = None;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "(" | "[" | "<" => {
                    if open_delimiter.is_none() {
                        open_delimiter = Some(child);
                    }
                }
                ")" | "]" | ">" => {
                    close_delimiter = Some(child);
                }
                "line_comment" | "multiline_comment" | "comment" => {}
                _ => {
                    if open_delimiter.is_some() {
                        if first_content.is_none() {
                            first_content = Some(child);
                        }
                        last_content = Some(child);
                    }
                }
            }
        }

        if let (Some(open), Some(first)) = (open_delimiter, first_content)
            && first.start_position().row == open.end_position().row
        {
            let message = match open.kind() {
                "(" => "Missing newline after \"(\"",
                "[" => "Missing newline after \"[\"",
                "<" => "Missing newline after \"<\"",
                _ => "Missing newline after opening delimiter",
            };
            ctx.report(open, message);
        }

        if let (Some(close), Some(last)) = (close_delimiter, last_content)
            && close.start_position().row == last.end_position().row
        {
            let message = match close.kind() {
                ")" => "Missing newline before \")\"",
                "]" => "Missing newline before \"]\"",
                ">" => "Missing newline before \">\"",
                _ => "Missing newline before closing delimiter",
            };
            ctx.report(close, message);
        }
    }
}
