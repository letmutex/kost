//! Built-in AST rule enforcing KDoc delimiter formatting (Ktlint rule 95).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_kdoc;

/// Rule that validates single-line and multiline KDoc delimiter structure.
pub struct KdocDelimiter;

impl AstRule for KdocDelimiter {
    fn id(&self) -> &'static str {
        "kdoc-delimiter"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if !is_kdoc(text) {
            return;
        }

        if node.start_position().row == node.end_position().row {
            check_single_line_kdoc(node, text, ctx);
        } else {
            check_multiline_kdoc(node, text, ctx);
        }
    }
}

fn check_single_line_kdoc(node: tree_sitter::Node, text: &str, ctx: &mut LintCtx) {
    let Some(inner) = text.strip_prefix("/**").and_then(|s| s.strip_suffix("*/")) else {
        return;
    };
    if !inner.is_empty() && (!inner.starts_with(' ') || !inner.ends_with(' ')) {
        ctx.report(
            node,
            "A single-line KDoc delimiter must be separated from content by a space",
        );
    }
}

fn check_multiline_kdoc(node: tree_sitter::Node, text: &str, ctx: &mut LintCtx) {
    let Some(first_nl) = text.find('\n') else {
        return;
    };
    let Some(last_nl) = text.rfind('\n') else {
        return;
    };

    let first_line = &text[..first_nl];
    if first_line.len() > 3 && !first_line[3..].trim().is_empty() {
        ctx.report(
            node,
            "A multiline KDoc should have '/**' on a separate line",
        );
    }

    let last_line = &text[last_nl + 1..];
    if let Some(before_closing) = last_line.strip_suffix("*/")
        && !before_closing.trim().is_empty()
    {
        ctx.report(node, "A multiline KDoc should have '*/' on a separate line");
    }

    if first_nl < last_nl {
        for line in text[first_nl + 1..last_nl].lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('*') {
                ctx.report(
                    node,
                    "A line in a multiline KDoc must start with leading asterisk",
                );
                break;
            }
        }
    }
}
