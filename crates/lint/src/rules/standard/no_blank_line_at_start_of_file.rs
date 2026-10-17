//! Built-in AST rule forbidding blank lines at the start of a file (Ktlint rule 66).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;

/// Rule that prohibits leading blank lines at the beginning of a source file.
pub struct NoBlankLineAtStartOfFile;

impl AstRule for NoBlankLineAtStartOfFile {
    fn id(&self) -> &'static str {
        "no-blank-line-at-start-of-file"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn check_node(&self, _node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if source.is_empty() {
            return;
        }

        let mut index = 0;
        while index < source.len() && matches!(source[index], b' ' | b'\t' | b'\r') {
            index += 1;
        }

        if index < source.len() && source[index] == b'\n' {
            ctx.report_range(
                TextRange {
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 1,
                },
                "Unexpected blank line(s)",
            );
        }
    }
}
