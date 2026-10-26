//! Built-in AST rule enforcing chain method continuation wrapping (Ktlint rule 45).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces method chains of 4 or more calls (or broken chains) are multiline.
pub struct ChainMethodContinuation;

impl AstRule for ChainMethodContinuation {
    fn id(&self) -> &'static str {
        "chain-method-continuation"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["navigation_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if !is_chain_root(ctx) {
            return;
        }

        let mut operators: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();
        collect_chain_operators(node, ctx.parent(), &mut operators);

        if operators.is_empty() {
            return;
        }

        let is_multiline_chain = operators.first().is_some_and(|first| {
            operators
                .last()
                .is_some_and(|last| first.start_position().row != last.start_position().row)
        }) || operators
            .iter()
            .any(|&op| spaces_before(op, source) == WhitespaceBefore::StartOfLine);

        let force_multiline = operators.len() >= 4 || is_multiline_chain;

        if force_multiline {
            for &operator_node in &operators {
                let operator_text = operator_node
                    .utf8_text(source)
                    .unwrap_or(operator_node.kind());

                let is_first_call_on_receiver_line = is_multiline_chain
                    && operator_node == operators[0]
                    && get_first_call_receiver(operator_node).is_some_and(|receiver| {
                        receiver.end_position().row == operator_node.start_position().row
                    });

                let is_valid_start = spaces_before(operator_node, source)
                    == WhitespaceBefore::StartOfLine
                    || is_on_closing_delimiter_line(operator_node, source)
                    || is_first_call_on_receiver_line;

                if !is_valid_start {
                    let message =
                        common::concat_str!("Expected newline before '", operator_text, "'");
                    ctx.report(operator_node, message);
                }

                if matches!(
                    spaces_after(operator_node, source),
                    WhitespaceAfter::EndOfLine { .. }
                ) {
                    let message =
                        common::concat_str!("Unexpected newline after '", operator_text, "'");
                    ctx.report(operator_node, message);
                }
            }
        }
    }
}

fn get_first_call_receiver(operator_node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = operator_node.parent()?;
    if current.kind() == "navigation_suffix" {
        current = current.parent()?;
    }
    if current.kind() == "navigation_expression" {
        return current.child(0);
    }
    None
}

fn is_on_closing_delimiter_line(operator_node: tree_sitter::Node, source: &[u8]) -> bool {
    let start = operator_node.start_byte();
    let mut i = start;
    while i > 0 && (source[i - 1] == b' ' || source[i - 1] == b'\t') {
        i -= 1;
    }
    if i == 0 {
        return false;
    }
    let delim = source[i - 1];
    if delim != b')' && delim != b'}' && delim != b']' && delim != b'"' {
        return false;
    }
    let mut j = i - 1;
    if delim == b'"' {
        if j >= 2 && source[j - 1] == b'"' && source[j - 2] == b'"' {
            j -= 2;
        } else {
            return false;
        }
    }
    while j > 0 {
        let b = source[j - 1];
        if b == b'\n' || b == b'\r' {
            return true;
        }
        if b != b' ' && b != b'\t' {
            return false;
        }
        j -= 1;
    }
    true
}

fn is_chain_root(ctx: &LintCtx) -> bool {
    let Some(parent) = ctx.parent() else {
        return true;
    };
    if parent.kind() == "navigation_expression" {
        return false;
    }
    if parent.kind() == "call_expression"
        && let Some(grandparent) = ctx.ancestor(2)
        && grandparent.kind() == "navigation_expression"
    {
        return false;
    }
    true
}

fn collect_chain_operators<'a>(
    root: tree_sitter::Node<'a>,
    parent_of_root: Option<tree_sitter::Node<'a>>,
    operators: &mut SmallVec<[tree_sitter::Node<'a>; 8]>,
) {
    let mut current = Some(root);
    let mut current_parent = parent_of_root;
    while let Some(current_node) = current {
        if current_node.kind() == "call_expression" {
            current_parent = Some(current_node);
            current = current_node.child(0);
        } else if current_node.kind() == "navigation_expression" {
            if is_call_navigation_with_parent(current_node, current_parent)
                && let Some(suffix_or_op) = current_node.child(1)
            {
                if suffix_or_op.kind() == "navigation_suffix" {
                    if let Some(operator_child) = suffix_or_op.child(0)
                        && matches!(operator_child.kind(), "." | "?.")
                    {
                        operators.push(operator_child);
                    }
                } else if matches!(suffix_or_op.kind(), "." | "?.") {
                    operators.push(suffix_or_op);
                }
            }
            current_parent = Some(current_node);
            current = current_node.child(0);
        } else {
            break;
        }
    }
    operators.reverse();
}

fn is_call_navigation_with_parent(
    nav_node: tree_sitter::Node,
    parent: Option<tree_sitter::Node>,
) -> bool {
    let Some(parent) = parent.or_else(|| nav_node.parent()) else {
        return false;
    };
    parent.kind() == "call_expression" && parent.child(0) == Some(nav_node)
}
