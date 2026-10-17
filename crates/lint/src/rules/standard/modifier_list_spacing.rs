//! Built-in AST rule enforcing single space between modifiers (Ktlint rule 32).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, spaces_after};

/// Rule that enforces exactly one space between successive modifiers on the same line.
pub struct ModifierListSpacing;

impl AstRule for ModifierListSpacing {
    fn id(&self) -> &'static str {
        "modifier-list-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["modifiers"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut prev: Option<tree_sitter::Node> = None;

        for child in node.children(&mut cursor) {
            if matches!(
                child.kind(),
                "annotation"
                    | "single_annotation"
                    | "file_annotation"
                    | "line_comment"
                    | "multiline_comment"
            ) {
                continue;
            }

            if let Some(prev_node) = prev
                && prev_node.end_position().row == child.start_position().row
            {
                match spaces_after(prev_node, source) {
                    WhitespaceAfter::Spaces(0) => {
                        ctx.report(prev_node, "Missing spacing after modifier");
                    }
                    WhitespaceAfter::Spaces(count) if count > 1 => {
                        ctx.report(prev_node, "Unexpected spacing after modifier");
                    }
                    _ => {}
                }
            }
            prev = Some(child);
        }

        if let Some(last) = prev
            && let Some(next_sibling) = node.next_sibling()
            && last.end_position().row == next_sibling.start_position().row
        {
            match spaces_after(last, source) {
                WhitespaceAfter::Spaces(0) => {
                    ctx.report(last, "Missing spacing after modifier");
                }
                WhitespaceAfter::Spaces(count) if count > 1 => {
                    ctx.report(last, "Unexpected spacing after modifier");
                }
                _ => {}
            }
        }
    }
}
