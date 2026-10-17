//! Built-in AST rule forbidding unparenthesized mixed logical operators (Ktlint rule 84).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that prohibits mixing '&&' and '||' in the same condition without disambiguating parentheses.
pub struct MixedConditionOperators;

impl AstRule for MixedConditionOperators {
    fn id(&self) -> &'static str {
        "mixed-condition-operators"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["disjunction_expression", "conjunction_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if is_child_of_logical_expression(node) {
            return;
        }

        let mut has_and = false;
        let mut has_or = false;

        collect_unparenthesized_logical_operators(node, &mut has_and, &mut has_or);

        if has_and && has_or {
            ctx.report(
                node,
                "A condition with mixed usage of '&&' and '||' is hard to read. Use parenthesis to clarify the (sub)condition.",
            );
        }
    }
}

fn is_child_of_logical_expression(node: tree_sitter::Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    matches!(
        parent.kind(),
        "disjunction_expression" | "conjunction_expression"
    )
}

fn collect_unparenthesized_logical_operators(
    node: tree_sitter::Node,
    has_and: &mut bool,
    has_or: &mut bool,
) {
    if *has_and && *has_or {
        return;
    }

    match node.kind() {
        "conjunction_expression" => *has_and = true,
        "disjunction_expression" => *has_or = true,
        _ => return,
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_unparenthesized_logical_operators(child, has_and, has_or);
    }
}
