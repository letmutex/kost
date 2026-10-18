//! Built-in AST rule forbidding multiple statements on a single line (Ktlint rule 63).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces individual statements are placed on separate lines.
pub struct StatementWrapping;

impl AstRule for StatementWrapping {
    fn id(&self) -> &'static str {
        "statement-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "statements",
            "class_body",
            "enum_class_body",
            "source_file",
            "import_list",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let is_enum_body = matches!(node.kind(), "enum_class_body" | "class_body");
        let mut cursor = node.walk();
        let mut prev_statement: Option<tree_sitter::Node> = None;

        for child in node.children(&mut cursor) {
            if !child.is_named() || child.is_error() {
                continue;
            }
            if matches!(
                child.kind(),
                "line_comment"
                    | "multiline_comment"
                    | "comment"
                    | "shebang_line"
                    | "annotation"
                    | "single_annotation"
                    | "file_annotation"
            ) {
                continue;
            }

            if child.kind() == "label" {
                if let Some(prev) = prev_statement
                    && child.start_position().row <= prev.end_position().row
                {
                    ctx.report(child, "Missing newline between statements");
                }
                prev_statement = Some(child);
                continue;
            }

            if child.kind() == "parenthesized_expression"
                && child
                    .prev_named_sibling()
                    .is_some_and(|s| matches!(s.kind(), "annotation" | "single_annotation"))
            {
                continue;
            }

            if is_enum_body
                && child.kind() == "enum_entry"
                && prev_statement.is_some_and(|p| p.kind() == "enum_entry")
            {
                prev_statement = Some(child);
                continue;
            }

            if let Some(prev) = prev_statement
                && child.start_position().row <= prev.end_position().row
                && prev.kind() != "label"
            {
                ctx.report(child, "Missing newline between statements");
            }

            prev_statement = Some(child);
        }
    }
}
