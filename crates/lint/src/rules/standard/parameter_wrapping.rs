//! Built-in AST rule enforcing parameter line wrapping (Ktlint rule 61).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::annotation_has_arguments;

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
                "parameter_modifiers" => {
                    check_parameter_modifiers(child, ctx);
                }
                "parameter" | "class_parameter" => {
                    let mut param_cursor = child.walk();
                    for param_child in child.children(&mut param_cursor) {
                        match param_child.kind() {
                            "=" => {
                                check_assignment_operator(param_child, ctx);
                            }
                            "modifiers" | "parameter_modifiers" => {
                                check_parameter_modifiers(param_child, ctx);
                            }
                            _ => {}
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

fn check_parameter_modifiers(modifiers_node: tree_sitter::Node, ctx: &mut LintCtx) {
    let mut cursor = modifiers_node.walk();
    let mut annotations = Vec::new();

    for child in modifiers_node.children(&mut cursor) {
        if matches!(child.kind(), "annotation" | "single_annotation") {
            annotations.push(child);
        }
    }

    if annotations.is_empty() {
        return;
    }

    for i in 1..annotations.len() {
        let prev_annot = annotations[i - 1];
        let curr_annot = annotations[i];
        if curr_annot.start_position().row == prev_annot.start_position().row {
            ctx.report(curr_annot, "Annotation should be on a separate line");
        }
    }

    let has_args = annotations.iter().any(|&a| annotation_has_arguments(a));
    if annotations.len() > 1 || has_args {
        let last_annotation = *annotations.last().unwrap();
        let mut found_next = false;
        let mut subsequent = last_annotation.next_sibling();
        while let Some(sub) = subsequent {
            if !matches!(sub.kind(), "line_comment" | "multiline_comment" | "comment") {
                if sub.start_position().row == last_annotation.end_position().row {
                    ctx.report(sub, "Parameter should start on a newline");
                }
                found_next = true;
                break;
            }
            subsequent = sub.next_sibling();
        }

        if !found_next {
            let mut next_opt = modifiers_node.next_sibling();
            while let Some(next) = next_opt {
                if !matches!(
                    next.kind(),
                    "line_comment" | "multiline_comment" | "comment"
                ) {
                    if next.start_position().row == last_annotation.end_position().row {
                        ctx.report(next, "Parameter should start on a newline");
                    }
                    break;
                }
                next_opt = next.next_sibling();
            }
        }
    }
}
