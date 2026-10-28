//! Built-in AST rule forbidding redundant Unit return types (Ktlint rule 75).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{BuiltinType, node_kind};

/// Rule that flags unnecessary explicit `Unit` return types on functions.
pub struct NoUnitReturn;

impl AstRule for NoUnitReturn {
    fn id(&self) -> &'static str {
        "no-unit-return"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration", "anonymous_function"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut seen_parameters = false;
        let mut seen_colon = false;
        let mut unit_type_node = None;

        for child in node.children(&mut cursor) {
            let kind = node_kind(child);
            if kind == "function_value_parameters" {
                seen_parameters = true;
            } else if seen_parameters && !seen_colon && kind == ":" {
                seen_colon = true;
            } else if seen_colon && unit_type_node.is_none() {
                if matches!(kind, "line_comment" | "multiline_comment" | "comment") {
                    continue;
                }
                if is_unit_type(child, source) {
                    unit_type_node = Some(child);
                } else {
                    return;
                }
            } else if unit_type_node.is_some() && kind == "function_body" {
                let mut body_cursor = child.walk();
                if child
                    .children(&mut body_cursor)
                    .any(|body_child| node_kind(body_child) == "=")
                {
                    return;
                }
            }
        }

        if let Some(type_node) = unit_type_node {
            ctx.report(type_node, "Unnecessary \"Unit\" return type");
        }
    }
}

fn is_unit_type(type_node: tree_sitter::Node, source: &[u8]) -> bool {
    if node_kind(type_node) == "nullable_type" {
        return false;
    }

    let Ok(text) = type_node.utf8_text(source) else {
        return false;
    };

    BuiltinType::Unit.matches_identifier(text.trim())
}
