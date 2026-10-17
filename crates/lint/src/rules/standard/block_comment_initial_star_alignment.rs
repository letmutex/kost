//! Built-in AST rule enforcing initial star alignment in block comments (Ktlint rule 91).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_kdoc;

/// Rule that enforces vertical alignment of leading asterisks in multiline block comments.
pub struct BlockCommentInitialStarAlignment;

impl AstRule for BlockCommentInitialStarAlignment {
    fn id(&self) -> &'static str {
        "block-comment-initial-star-alignment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if !text.starts_with("/*") || !text.ends_with("*/") || is_kdoc(text) {
            return;
        }

        let expected_column = node.start_position().column + 1;
        if has_misaligned_initial_star(text, expected_column) {
            ctx.report(
                node,
                "Initial star should align with start of block comment",
            );
        }
    }
}

fn has_misaligned_initial_star(text: &str, expected_column: usize) -> bool {
    for line in text.split('\n').skip(1) {
        let trimmed = line.trim_start_matches([' ', '\t']);
        let leading_spaces = line.len() - trimmed.len();
        if leading_spaces > 0 && trimmed.starts_with('*') && leading_spaces != expected_column {
            return true;
        }
    }
    false
}
