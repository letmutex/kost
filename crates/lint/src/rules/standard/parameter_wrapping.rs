//! Built-in AST rule enforcing parameter line wrapping (Ktlint rule 61).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces proper line wrapping for individual parameter components.
pub struct ParameterWrapping;

impl AstRule for ParameterWrapping {
    fn id(&self) -> &'static str {
        "parameter-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "function_value_parameters",
            "class_parameters",
            "primary_constructor",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "=" => {
                    check_assignment_operator(child, ctx);
                }
                "parameter" | "class_parameter" => {
                    let mut param_cursor = child.walk();
                    for param_child in child.children(&mut param_cursor) {
                        if param_child.kind() == "=" {
                            check_assignment_operator(param_child, ctx);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn check_assignment_operator(eq_node: tree_sitter::Node, ctx: &mut LintCtx) {
    let mut prev_opt = eq_node.prev_sibling();
    while let Some(prev) = prev_opt {
        if !matches!(
            prev.kind(),
            "line_comment" | "multiline_comment" | "comment"
        ) {
            if eq_node.start_position().row > prev.end_position().row {
                ctx.report(eq_node, "Unexpected newline before '='");
            }
            break;
        }
        prev_opt = prev.prev_sibling();
    }
}
