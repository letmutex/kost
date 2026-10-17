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

        let is_any_on_newline = operators
            .iter()
            .any(|&op| spaces_before(op, source) == WhitespaceBefore::StartOfLine);

        let force_multiline = operators.len() >= 4 || is_any_on_newline;

        if force_multiline {
            for &operator_node in &operators {
                let operator_text = operator_node
                    .utf8_text(source)
                    .unwrap_or(operator_node.kind());

                if spaces_before(operator_node, source) != WhitespaceBefore::StartOfLine {
                    let message =
                        crate::concat_str!("Expected newline before '", operator_text, "'");
                    ctx.report(operator_node, message);
                }

                if matches!(
                    spaces_after(operator_node, source),
                    WhitespaceAfter::EndOfLine { .. }
                ) {
                    let message =
                        crate::concat_str!("Unexpected newline after '", operator_text, "'");
                    ctx.report(operator_node, message);
                }
            }
        }
    }
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
