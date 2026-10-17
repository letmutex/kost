//! Built-in AST rule enforcing expression operand wrapping (Ktlint rule 51).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces clean line wrapping of operands in multiline binary expressions.
pub struct ExpressionOperandWrapping;

impl AstRule for ExpressionOperandWrapping {
    fn id(&self) -> &'static str {
        "expression-operand-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "additive_expression",
            "multiplicative_expression",
            "comparison_expression",
            "equality_expression",
            "conjunction_expression",
            "disjunction_expression",
            "infix_expression",
            "elvis_expression",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.start_position().row == node.end_position().row {
            return;
        }

        if node
            .parent()
            .is_some_and(|parent| parent.kind() == node.kind())
        {
            return;
        }

        let mut operands: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();
        collect_leaf_operands(node, node.kind(), &mut operands);

        if operands.len() <= 1 {
            return;
        }

        for index in 1..operands.len() {
            let previous_operand = operands[index - 1];
            let current_operand = operands[index];

            if current_operand.start_position().row == previous_operand.end_position().row {
                ctx.report(
                    current_operand,
                    "Expression operand should be on a separate line",
                );
            }
        }
    }
}

fn collect_leaf_operands<'a>(
    node: tree_sitter::Node<'a>,
    expected_kind: &str,
    operands: &mut SmallVec<[tree_sitter::Node<'a>; 8]>,
) {
    if node.kind() == expected_kind {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named()
                && !matches!(
                    child.kind(),
                    "line_comment" | "multiline_comment" | "comment"
                )
            {
                collect_leaf_operands(child, expected_kind, operands);
            }
        }
    } else {
        operands.push(node);
    }
}
