//! Built-in AST rule enforcing argument list wrapping (Ktlint rule 42).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that verifies multiline argument lists place each argument and closing parenthesis on separate lines.
pub struct ArgumentListWrapping;

impl AstRule for ArgumentListWrapping {
    fn id(&self) -> &'static str {
        "argument-list-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["value_arguments"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let mut opening_parenthesis = None;
        let mut closing_parenthesis = None;
        let mut argument_nodes: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "(" => opening_parenthesis = Some(child),
                ")" => closing_parenthesis = Some(child),
                "value_argument" => argument_nodes.push(child),
                _ => {}
            }
        }

        if argument_nodes.is_empty() {
            return;
        }

        let Some(opening_paren) = opening_parenthesis else {
            return;
        };

        let first_argument = argument_nodes[0];
        if first_argument.start_position().row == opening_paren.end_position().row {
            ctx.report(
                first_argument,
                "Argument should be on a separate line (unless all arguments can fit a single line)",
            );
        }

        for index in 1..argument_nodes.len() {
            let previous_argument = argument_nodes[index - 1];
            let current_argument = argument_nodes[index];
            if current_argument.start_position().row == previous_argument.end_position().row {
                ctx.report(
                    current_argument,
                    "Argument should be on a separate line (unless all arguments can fit a single line)",
                );
            }
        }

        if let Some(closing_paren) = closing_parenthesis
            && let Some(&last_argument) = argument_nodes.last()
            && closing_paren.start_position().row == last_argument.end_position().row
        {
            ctx.report(closing_paren, "Missing newline before \")\"");
        }
    }
}
