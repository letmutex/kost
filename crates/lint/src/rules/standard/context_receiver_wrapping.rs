//! Built-in AST rule enforcing context receiver wrapping (Ktlint rule 49).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces context receivers are followed by a newline and multiline receivers are wrapped cleanly.
pub struct ContextReceiverWrapping;

impl AstRule for ContextReceiverWrapping {
    fn id(&self) -> &'static str {
        "context-receiver-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["context_receiver"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(parent) = ctx.parent() else {
            return;
        };
        if !matches!(
            parent.kind(),
            "class_declaration"
                | "function_declaration"
                | "property_declaration"
                | "secondary_constructor"
                | "typealias_declaration"
                | "modifiers"
        ) {
            return;
        }

        let mut receivers: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();
        let mut closing_parenthesis = None;
        let mut has_colon = false;

        collect_receivers(
            node,
            &mut receivers,
            &mut closing_parenthesis,
            &mut has_colon,
        );

        if has_colon || receivers.is_empty() {
            return;
        }

        if !matches!(
            spaces_after(node, source),
            WhitespaceAfter::EndOfLine { .. }
        ) {
            ctx.report(node, "Expected a newline after the context receiver");
        }

        if node.start_position().row == node.end_position().row {
            return;
        }

        for receiver in receivers {
            if spaces_before(receiver, source) != WhitespaceBefore::StartOfLine {
                ctx.report(receiver, "Newline expected before context receiver");
            }
        }

        if let Some(rpar) = closing_parenthesis
            && spaces_before(rpar, source) != WhitespaceBefore::StartOfLine
        {
            ctx.report(rpar, "Newline expected before closing parenthesis");
        }
    }
}

fn collect_receivers<'a>(
    node: tree_sitter::Node<'a>,
    receivers: &mut SmallVec<[tree_sitter::Node<'a>; 8]>,
    closing_parenthesis: &mut Option<tree_sitter::Node<'a>>,
    has_colon: &mut bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            ")" => *closing_parenthesis = Some(child),
            ":" => *has_colon = true,
            "(" | "context" | "," | "comment" | "line_comment" | "multiline_comment" => {}

            _ => {
                if child.is_named() {
                    receivers.push(child);
                }
            }
        }
    }
}
