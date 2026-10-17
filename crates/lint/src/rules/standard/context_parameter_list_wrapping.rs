//! Built-in AST rule enforcing context parameter list wrapping (Ktlint rule 48).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, spaces_before};

use smallvec::SmallVec;

/// Rule that enforces multiline context parameter lists place each parameter and closing parenthesis on separate lines.
pub struct ContextParameterListWrapping;

impl AstRule for ContextParameterListWrapping {
    fn id(&self) -> &'static str {
        "context-parameter-list-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["context_receiver"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        let mut parameters: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();
        let mut closing_parenthesis = None;
        let mut has_colon = false;

        collect_context_parameters(
            node,
            &mut parameters,
            &mut closing_parenthesis,
            &mut has_colon,
        );

        if !has_colon || parameters.is_empty() {
            return;
        }

        for parameter in parameters {
            if spaces_before(parameter, source) != WhitespaceBefore::StartOfLine {
                ctx.report(parameter, "Context parameter should be on a separate line");
            }
        }

        if let Some(rpar) = closing_parenthesis
            && spaces_before(rpar, source) != WhitespaceBefore::StartOfLine
        {
            ctx.report(rpar, "Newline expected before closing parenthesis");
        }
    }
}

fn collect_context_parameters<'a>(
    node: tree_sitter::Node<'a>,
    parameters: &mut SmallVec<[tree_sitter::Node<'a>; 8]>,
    closing_parenthesis: &mut Option<tree_sitter::Node<'a>>,
    has_colon: &mut bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            ")" => *closing_parenthesis = Some(child),
            ":" => *has_colon = true,
            "simple_identifier"
                if child
                    .next_sibling()
                    .is_some_and(|sibling| sibling.kind() == ":") =>
            {
                parameters.push(find_parameter_start_node(child));
            }

            _ => {}
        }
    }
}

fn find_parameter_start_node<'a>(identifier: tree_sitter::Node<'a>) -> tree_sitter::Node<'a> {
    let mut current = identifier;
    while let Some(prev) = current.prev_sibling() {
        if prev.kind() == "(" || prev.kind() == "," {
            break;
        }
        current = prev;
    }
    current
}
