//! Built-in AST rule enforcing spacing in function type references (Ktlint rule 31).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around receiver dot and `->` in function types.
pub struct FunctionTypeReferenceSpacing;

impl AstRule for FunctionTypeReferenceSpacing {
    fn id(&self) -> &'static str {
        "function-type-reference-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_type"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "." => check_receiver_dot(child, source, ctx),
                "->" => check_arrow(child, source, ctx),
                _ => {}
            }
        }
    }
}

fn check_receiver_dot(dot: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    match spaces_before(dot, source) {
        WhitespaceBefore::Spaces(count) if count > 0 => {
            ctx.report(dot, "Unexpected spacing before '.'");
        }
        WhitespaceBefore::StartOfLine => {
            ctx.report(dot, "Unexpected spacing before '.'");
        }
        _ => {}
    }

    match spaces_after(dot, source) {
        WhitespaceAfter::Spaces(count) if count > 0 => {
            ctx.report(dot, "Unexpected spacing after '.'");
        }
        WhitespaceAfter::EndOfLine { .. } => {
            ctx.report(dot, "Unexpected spacing after '.'");
        }
        _ => {}
    }
}

fn check_arrow(arrow: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    match spaces_before(arrow, source) {
        WhitespaceBefore::Spaces(0) => {
            ctx.report(arrow, "Missing spacing before '->'");
        }
        WhitespaceBefore::Spaces(count) if count > 1 => {
            ctx.report(arrow, "Unexpected spacing before '->'");
        }
        _ => {}
    }

    match spaces_after(arrow, source) {
        WhitespaceAfter::Spaces(0) => {
            ctx.report(arrow, "Missing spacing after '->'");
        }
        WhitespaceAfter::Spaces(count) if count > 1 => {
            ctx.report(arrow, "Unexpected spacing after '->'");
        }
        WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
            ctx.report(arrow, "Unexpected spacing after '->'");
        }
        _ => {}
    }
}
