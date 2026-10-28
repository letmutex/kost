//! Built-in AST rule enforcing spacing around curly braces (Ktlint rule 16).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, node_kind, spaces_after, spaces_before};

/// Rule that enforces proper spacing around curly braces.
pub struct CurlySpacing;

impl AstRule for CurlySpacing {
    fn id(&self) -> &'static str {
        "curly-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["{", "}"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node_kind(node) == "{" {
            check_opening_curly(node, source, ctx);
        } else {
            check_closing_curly(node, source, ctx);
        }
    }
}

fn is_preceded_by_opening_delimiter(node: tree_sitter::Node, source: &[u8]) -> bool {
    let start = node.start_byte();
    if start == 0 {
        return false;
    }
    matches!(source[start - 1], b'(' | b'[' | b'"' | b'\'' | b'@')
}

fn check_opening_curly(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    match spaces_before(node, source) {
        WhitespaceBefore::StartOfLine => {}
        WhitespaceBefore::Spaces(0) if !is_preceded_by_opening_delimiter(node, source) => {
            ctx.report(node, "Missing spacing before '{'");
        }
        WhitespaceBefore::Spaces(n) if n > 1 => {
            ctx.report(node, "Unexpected spacing before '{'");
        }
        _ => {}
    }

    let start = node.end_byte();
    let rest = &source[start..];
    let line_len = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
    if !rest[..line_len].contains(&b'}') {
        return;
    }

    let Some(closing) = find_closing_brace(node, ctx.parent()) else {
        return;
    };

    if closing.start_position().row == node.end_position().row {
        let Some(inside_span) = source.get(node.end_byte()..closing.start_byte()) else {
            return;
        };
        if inside_span.iter().all(|b| b.is_ascii_whitespace()) {
            if !inside_span.is_empty() {
                ctx.report(node, "Unexpected spacing between '{' and '}'");
            }
        } else {
            match spaces_after(node, source) {
                WhitespaceAfter::Spaces(0) => {
                    ctx.report(node, "Missing spacing after '{'");
                }
                WhitespaceAfter::Spaces(n) if n > 1 => {
                    ctx.report(node, "Unexpected spacing after '{'");
                }
                _ => {}
            }
            match spaces_before(closing, source) {
                WhitespaceBefore::Spaces(0) => {
                    ctx.report(closing, "Missing spacing before '}'");
                }
                WhitespaceBefore::Spaces(n) if n > 1 => {
                    ctx.report(closing, "Unexpected spacing before '}'");
                }
                _ => {}
            }
        }
    }
}

fn is_followed_by_control_flow(node: tree_sitter::Node, ctx: &LintCtx) -> bool {
    let check_kind =
        |kind: &str| matches!(kind, "else" | "while" | "catch_block" | "finally_block");
    if let Some(next) = node.next_sibling()
        && check_kind(node_kind(next))
    {
        return true;
    }
    if let Some(parent) = ctx.parent()
        && matches!(node_kind(parent), "control_structure_body" | "catch_block")
        && let Some(parent_next) = parent.next_sibling()
        && check_kind(node_kind(parent_next))
    {
        return true;
    }
    false
}

fn check_closing_curly(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    if let WhitespaceAfter::Spaces(count) = spaces_after(node, source)
        && is_followed_by_control_flow(node, ctx)
    {
        if count == 0 {
            ctx.report(node, "Missing spacing after '}'");
        } else if count > 1 {
            ctx.report(node, "Unexpected spacing after '}'");
        }
    }
}

fn find_closing_brace<'a>(
    opening: tree_sitter::Node<'a>,
    parent: Option<tree_sitter::Node<'a>>,
) -> Option<tree_sitter::Node<'a>> {
    let parent = parent?;
    let mut cursor = parent.walk();
    parent
        .children(&mut cursor)
        .find(|&child| node_kind(child) == "}" && child.start_byte() > opening.end_byte())
}
