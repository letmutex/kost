//! Built-in AST rule detecting unnecessary semicolons.

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that detects unnecessary semicolons in Kotlin source code.
pub struct NoSemi;

impl AstRule for NoSemi {
    fn id(&self) -> &'static str {
        "no-semi"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[";", "semicolon"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if is_required_enum_semicolon(node, source) || is_empty_control_body(node) {
            return;
        }

        ctx.report(node, "Unnecessary semicolon");
    }
}

/// Checks if this semicolon is the required separator between enum entries and class members.
fn is_required_enum_semicolon(node: tree_sitter::Node, _source: &[u8]) -> bool {
    if let Some(parent) = node.parent()
        && parent.kind() == "enum_class_body"
    {
        // If this semicolon is preceded by another semicolon, it is redundant
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "line_comment" || sibling.kind() == "multiline_comment" {
                prev = sibling.prev_sibling();
                continue;
            }
            if sibling.kind() == ";" || sibling.kind() == "semicolon" {
                return false;
            }
            break;
        }

        // If there are member declarations following the semicolon, it is required in Kotlin
        let mut next_sibling = node.next_sibling();
        while let Some(sibling) = next_sibling {
            if sibling.is_named()
                && sibling.kind() != "line_comment"
                && sibling.kind() != "multiline_comment"
            {
                return true;
            }
            next_sibling = sibling.next_sibling();
        }
    }
    false
}

/// Checks if this semicolon acts as the empty body of a control structure (e.g. while (cond);).
fn is_empty_control_body(node: tree_sitter::Node) -> bool {
    if let Some(parent) = node.parent() {
        let parent_kind = parent.kind();
        if matches!(
            parent_kind,
            "while_statement" | "do_while_statement" | "for_statement"
        ) {
            let mut cursor = parent.walk();
            return !parent
                .children(&mut cursor)
                .any(|c| c.kind() == "control_structure_body");
        }
        if parent_kind == "if_expression" {
            let mut cursor = parent.walk();
            let mut has_consequence = false;
            let mut has_alternative = false;
            let mut seen_else = false;
            let mut is_before_else = true;

            for child in parent.children(&mut cursor) {
                if child.id() == node.id() {
                    is_before_else = !seen_else;
                } else if child.kind() == "else" {
                    seen_else = true;
                } else if child.kind() == "control_structure_body" {
                    if seen_else {
                        has_alternative = true;
                    } else {
                        has_consequence = true;
                    }
                }
            }

            return (is_before_else && !has_consequence) || (!is_before_else && !has_alternative);
        }
    }
    false
}
