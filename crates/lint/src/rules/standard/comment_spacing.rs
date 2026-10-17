//! Built-in AST rule enforcing spacing around comments (Ktlint rule 92).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, is_kdoc, spaces_before};

/// Rule that enforces consistent spacing before and after comment delimiters.
pub struct CommentSpacing;

impl AstRule for CommentSpacing {
    fn id(&self) -> &'static str {
        "comment-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        let trimmed_text = text.trim_end_matches(['\r', '\n']);
        if trimmed_text.starts_with("//") {
            check_line_comment(node, trimmed_text, source, ctx);
        } else if trimmed_text.starts_with("/*")
            && trimmed_text.ends_with("*/")
            && !is_kdoc(trimmed_text)
        {
            check_block_comment(node, trimmed_text, source, ctx);
        }
    }
}

fn check_line_comment(node: tree_sitter::Node, text: &str, source: &[u8], ctx: &mut LintCtx) {
    if spaces_before(node, source) == WhitespaceBefore::Spaces(0) {
        ctx.report(node, "Missing space before //");
    }

    let content = &text[2..];
    if content.is_empty()
        || content.starts_with('/')
        || content.starts_with('!')
        || content.starts_with("noinspection")
        || content.starts_with("region")
        || content.starts_with("endregion")
        || content.starts_with("language=")
        || content.starts_with("suppress")
    {
        return;
    }

    if !content.starts_with(' ') && !content.starts_with('\t') {
        ctx.report(node, "Missing space after //");
    }
}

fn check_block_comment(node: tree_sitter::Node, text: &str, source: &[u8], ctx: &mut LintCtx) {
    if spaces_before(node, source) == WhitespaceBefore::Spaces(0) {
        ctx.report(node, "Missing space before /*");
    }

    if node.start_position().row != node.end_position().row {
        return;
    }

    if text.len() <= 4 || text == "/* */" {
        return;
    }

    let inner = &text[2..text.len() - 2];
    if inner.is_empty() {
        return;
    }

    if !inner.starts_with(' ') && !inner.starts_with('\t') && !inner.starts_with('*') {
        ctx.report(node, "Missing space after /*");
    }

    if !inner.ends_with(' ') && !inner.ends_with('\t') && !inner.ends_with('*') {
        ctx.report(node, "Missing space before */");
    }
}
