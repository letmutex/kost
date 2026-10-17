//! Built-in AST rule forbidding blank lines in chained method calls (Ktlint rule 69).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that forbids blank lines between chained method calls.
pub struct NoBlankLinesInChainedMethodCalls;

impl AstRule for NoBlankLinesInChainedMethodCalls {
    fn id(&self) -> &'static str {
        "no-blank-lines-in-chained-method-calls"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["navigation_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut receiver: Option<tree_sitter::Node> = None;
        let mut suffix_or_operator: Option<tree_sitter::Node> = None;
        let mut last_end_row = 0;
        let mut has_blank_before_operator = false;

        for child in node.children(&mut cursor) {
            if receiver.is_none() {
                if child.is_named() {
                    receiver = Some(child);
                    last_end_row = child.end_position().row;
                }
            } else if matches!(child.kind(), "." | "?." | "navigation_suffix") {
                if child.start_position().row > last_end_row + 1 {
                    has_blank_before_operator = true;
                }
                suffix_or_operator = Some(child);
                break;
            } else {
                if child.start_position().row > last_end_row + 1 {
                    has_blank_before_operator = true;
                }
                last_end_row = last_end_row.max(child.end_position().row);
            }
        }

        let Some(suffix_node) = suffix_or_operator else {
            return;
        };

        let mut operator_node = suffix_node;
        let mut callee: Option<tree_sitter::Node> = None;
        let mut has_blank_before_callee = false;

        if suffix_node.kind() == "navigation_suffix" {
            let mut suffix_cursor = suffix_node.walk();
            let mut seen_operator = false;
            let mut op_last_row = 0;

            for child in suffix_node.children(&mut suffix_cursor) {
                if !seen_operator {
                    if matches!(child.kind(), "." | "?.") {
                        operator_node = child;
                        seen_operator = true;
                        op_last_row = child.end_position().row;
                    }
                } else if child.is_named() {
                    if child.start_position().row > op_last_row + 1 {
                        has_blank_before_callee = true;
                    }
                    callee = Some(child);
                    break;
                } else {
                    if child.start_position().row > op_last_row + 1 {
                        has_blank_before_callee = true;
                    }
                    op_last_row = op_last_row.max(child.end_position().row);
                }
            }
        }

        if has_blank_before_operator {
            ctx.report(
                operator_node,
                "Unexpected blank line in chained method calls",
            );
        }

        if has_blank_before_callee && let Some(callee_node) = callee {
            ctx.report(callee_node, "Unexpected blank line in chained method calls");
        }
    }
}
