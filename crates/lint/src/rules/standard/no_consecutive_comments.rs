//! Built-in AST rule forbidding consecutive comments (Ktlint rule 71).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::CommentType;
use crate::util::is_kdoc;

/// Rule that disallows consecutive comments unless properly separated.
pub struct NoConsecutiveComments;

impl AstRule for NoConsecutiveComments {
    fn id(&self) -> &'static str {
        "no-consecutive-comments"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };
        let current_type = classify_comment(text);

        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let prefix = &source[..start_byte];
        let (prev_byte_index, newlines_between) = find_prev_non_whitespace(prefix);

        if let Some(prev_byte) = prev_byte_index
            && let Some((prev_type, prev_start, prev_end)) = ctx.last_comment
            && prev_byte >= prev_start
            && prev_byte < prev_end
        {
            evaluate_consecutive_comments(node, current_type, prev_type, newlines_between, ctx);
        }

        ctx.last_comment = Some((current_type, start_byte, end_byte));
    }
}

fn classify_comment(text: &str) -> CommentType {
    let trimmed = text.trim_start();
    if trimmed.starts_with("//") {
        CommentType::Eol
    } else if is_kdoc(trimmed) {
        CommentType::Kdoc
    } else {
        CommentType::Block
    }
}

fn find_prev_non_whitespace(prefix: &[u8]) -> (Option<usize>, usize) {
    let mut newlines = 0;
    let mut cursor = prefix.len();

    while cursor > 0 {
        cursor -= 1;
        let b = prefix[cursor];
        if b == b'\n' {
            newlines += 1;
        } else if b != b' ' && b != b'\t' && b != b'\r' {
            return (Some(cursor), newlines);
        }
    }
    (None, newlines)
}

fn evaluate_consecutive_comments(
    node: tree_sitter::Node,
    current: CommentType,
    previous: CommentType,
    newlines: usize,
    ctx: &mut LintCtx,
) {
    if previous == CommentType::Kdoc && current == CommentType::Kdoc {
        let msg = crate::concat_str!(
            current.description(),
            " may not be preceded by ",
            previous.description()
        );
        ctx.report(node, msg);
    } else if previous == CommentType::Kdoc && current != CommentType::Kdoc {
        let msg = crate::concat_str!(
            current.description(),
            " may not be preceded by ",
            previous.description(),
            ". Reversed order is allowed though when separated by a newline."
        );
        ctx.report(node, msg);
    } else if previous == CommentType::Block && current == CommentType::Block {
        if newlines <= 1 {
            let msg = crate::concat_str!(
                current.description(),
                " may not be preceded by ",
                previous.description()
            );
            ctx.report(node, msg);
        }
    } else if previous == CommentType::Eol && current == CommentType::Eol {
        // Allowed
    } else if previous != current && newlines > 1 {
        // Allowed when separated by a blank line
    } else {
        let msg = crate::concat_str!(
            current.description(),
            " may not be preceded by ",
            previous.description(),
            " unless separated by a blank line"
        );
        ctx.report(node, msg);
    }
}
