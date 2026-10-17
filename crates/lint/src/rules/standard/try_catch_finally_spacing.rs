//! Built-in AST rule enforcing spacing around try-catch-finally blocks (Ktlint rule 37).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces canonical spacing and placement for catch and finally blocks.
pub struct TryCatchFinallySpacing;

impl AstRule for TryCatchFinallySpacing {
    fn id(&self) -> &'static str {
        "try-catch-finally-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["catch_block", "finally_block"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "catch_block" {
            check_catch_block(node, source, ctx);
        } else {
            check_finally_block(node, source, ctx);
        }
    }
}

fn check_catch_block(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let mut cursor = node.walk();
    let mut catch_node = None;
    let mut lbrace_node = None;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "catch" => catch_node = Some(child),
            "{" => lbrace_node = Some(child),
            _ => {}
        }
    }

    if let Some(catch_tok) = catch_node {
        match spaces_before(catch_tok, source) {
            WhitespaceBefore::Spaces(0) => {
                ctx.report(catch_tok, "Missing spacing before 'catch'");
            }
            WhitespaceBefore::Spaces(count) if count > 1 => {
                ctx.report(catch_tok, "Unexpected spacing before 'catch'");
            }
            WhitespaceBefore::StartOfLine => {
                let is_preceded_by_comment = node
                    .prev_sibling()
                    .map(|prev| matches!(prev.kind(), "line_comment" | "multiline_comment"))
                    .unwrap_or(false);
                if !is_preceded_by_comment {
                    ctx.report(catch_tok, "Unexpected line break before 'catch'");
                }
            }
            _ => {}
        }

        match spaces_after(catch_tok, source) {
            WhitespaceAfter::Spaces(0) => {
                ctx.report(catch_tok, "Missing spacing after 'catch'");
            }
            WhitespaceAfter::Spaces(count) if count > 1 => {
                ctx.report(catch_tok, "Unexpected spacing after 'catch'");
            }
            WhitespaceAfter::EndOfLine { .. } => {
                ctx.report(catch_tok, "Unexpected spacing after 'catch'");
            }
            _ => {}
        }
    }

    if let Some(lbrace) = lbrace_node {
        match spaces_before(lbrace, source) {
            WhitespaceBefore::Spaces(0) => {
                ctx.report(lbrace, "Missing spacing before '{'");
            }
            WhitespaceBefore::Spaces(count) if count > 1 => {
                ctx.report(lbrace, "Unexpected spacing before '{'");
            }
            WhitespaceBefore::StartOfLine => {
                ctx.report(lbrace, "Unexpected line break before '{'");
            }
            _ => {}
        }
    }
}

fn check_finally_block(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let mut cursor = node.walk();
    let mut finally_node = None;

    for child in node.children(&mut cursor) {
        if child.kind() == "finally" {
            finally_node = Some(child);
            break;
        }
    }

    let Some(finally_tok) = finally_node else {
        return;
    };

    match spaces_before(finally_tok, source) {
        WhitespaceBefore::Spaces(0) => {
            ctx.report(finally_tok, "Missing spacing before 'finally'");
        }
        WhitespaceBefore::Spaces(count) if count > 1 => {
            ctx.report(finally_tok, "Unexpected spacing before 'finally'");
        }
        WhitespaceBefore::StartOfLine => {
            let is_preceded_by_comment = node
                .prev_sibling()
                .map(|prev| matches!(prev.kind(), "line_comment" | "multiline_comment"))
                .unwrap_or(false);
            if !is_preceded_by_comment {
                ctx.report(finally_tok, "Unexpected line break before 'finally'");
            }
        }
        _ => {}
    }

    match spaces_after(finally_tok, source) {
        WhitespaceAfter::Spaces(0) => {
            ctx.report(finally_tok, "Missing spacing after 'finally'");
        }
        WhitespaceAfter::Spaces(count) if count > 1 => {
            ctx.report(finally_tok, "Unexpected spacing after 'finally'");
        }
        WhitespaceAfter::EndOfLine { .. } => {
            ctx.report(finally_tok, "Unexpected spacing after 'finally'");
        }
        _ => {}
    }
}
