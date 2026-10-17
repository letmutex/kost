//! Built-in AST rule enforcing trailing newline at end of file (Ktlint rule 104).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

/// Rule that enforces that files end with a newline character.
pub struct FinalNewline;

impl AstRule for FinalNewline {
    fn id(&self) -> &'static str {
        "final-newline"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn check_node(&self, _node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if !ctx.insert_final_newline() || source.is_empty() || source.ends_with(b"\n") {
            return;
        }

        let mut line_number = 1;
        let mut last_line_start = 0;

        for (index, &byte) in source.iter().enumerate() {
            if byte == b'\n' {
                line_number += 1;
                last_line_start = index + 1;
            }
        }

        let last_line = &source[last_line_start..];
        let char_count =
            std::str::from_utf8(last_line).map_or_else(|_| last_line.len(), |s| s.chars().count());
        let last_col = (char_count + 1) as u32;

        ctx.report_range(
            TextRange::new(line_number, last_col, line_number, last_col),
            "File must end with a newline (\\n)",
        );
    }
}
