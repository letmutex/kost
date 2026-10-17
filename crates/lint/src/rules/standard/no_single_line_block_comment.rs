//! Built-in AST rule forbidding single-line block comments (Ktlint rule 74).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that flags single-line block comments (`/* ... */`) that should be written with `//`.
pub struct NoSingleLineBlockComment;

impl AstRule for NoSingleLineBlockComment {
    fn id(&self) -> &'static str {
        "no-single-line-block-comment"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row != node.end_position().row {
            return;
        }

        if let WhitespaceAfter::Spaces(_) = spaces_after(node, source) {
            return;
        }

        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if text.starts_with("/*") && text.ends_with("*/") {
            if text.starts_with("/**") && text != "/**/" {
                return;
            }
            ctx.report(node, "Replace the block comment with an EOL comment");
        }
    }
}
