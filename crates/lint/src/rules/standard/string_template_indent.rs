//! Built-in AST rule enforcing proper indentation on multiline raw strings (Ktlint rule 88).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces newlines and consistent indentation for multiline raw string templates.
pub struct StringTemplateIndent;

impl AstRule for StringTemplateIndent {
    fn id(&self) -> &'static str {
        "string-template-indent"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["string_literal", "multiline_string_literal"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if !text.starts_with("\"\"\"") || !text.ends_with("\"\"\"") || !text.contains('\n') {
            return;
        }

        if !is_followed_by_trim_indent(node, source) {
            return;
        }

        check_preceding_newline(node, source, ctx);
        check_quotes_newlines(node, text, ctx);
        check_mixed_indentation(node, text, ctx);
    }
}

fn is_followed_by_trim_indent(node: tree_sitter::Node, source: &[u8]) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "navigation_expression" {
        return false;
    }

    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() == "navigation_suffix" {
            let mut suffix_cursor = child.walk();
            for suffix_child in child.children(&mut suffix_cursor) {
                if suffix_child.kind() == "simple_identifier"
                    && let Ok(name) = suffix_child.utf8_text(source)
                    && (name == "trimIndent" || name == "trimMargin")
                {
                    return true;
                }
            }
        }
    }

    false
}

fn check_preceding_newline(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if is_preceded_by_return_or_equals(node) {
        return;
    }

    let start_byte = node.start_byte();
    let line_start = find_line_start(source, start_byte);
    let leading_slice = &source[line_start..start_byte];

    if !leading_slice.iter().all(|&b| b == b' ' || b == b'\t') {
        ctx.report(node, "Expected newline before multiline string template");
    }
}

fn is_preceded_by_return_or_equals(node: tree_sitter::Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "jump_expression" || parent.kind() == "function_declaration" {
            return true;
        }
        if matches!(
            parent.kind(),
            "statements" | "class_body" | "value_argument"
        ) {
            break;
        }
        current = parent.parent();
    }
    false
}

fn find_line_start(source: &[u8], offset: usize) -> usize {
    let mut i = offset;
    while i > 0 && source[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

fn check_quotes_newlines(node: tree_sitter::Node, text: &str, ctx: &mut LintCtx) {
    let without_opening = &text[3..];
    if let Some(first_newline) = without_opening.find('\n') {
        let first_line = &without_opening[..first_newline];
        if !first_line.trim().is_empty() {
            ctx.report(
                node,
                "Missing newline after the opening quotes of the raw string literal",
            );
        }
    }

    let without_closing = &text[..text.len() - 3];
    if let Some(last_newline) = without_closing.rfind('\n') {
        let last_line = &without_closing[last_newline + 1..];
        if !last_line.trim().is_empty() {
            ctx.report(
                node,
                "Missing newline before the closing quotes of the raw string literal",
            );
        }
    }
}

fn check_mixed_indentation(node: tree_sitter::Node, text: &str, ctx: &mut LintCtx) {
    let mut has_spaces = false;
    let mut has_tabs = false;

    for line in text.lines().skip(1) {
        if has_spaces && has_tabs {
            break;
        }
        let trimmed_end = line.trim_end_matches("\"\"\"");
        for c in trimmed_end.chars().take_while(|c| c.is_whitespace()) {
            if c == ' ' {
                has_spaces = true;
            } else if c == '\t' {
                has_tabs = true;
            }
        }
    }

    if has_spaces && has_tabs {
        ctx.report(
            node,
            "Indentation of multiline raw string literal should not contain both tab(s) and space(s)",
        );
    }
}
