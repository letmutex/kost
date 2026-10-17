//! Built-in AST rule forbidding redundant Unit return types (Ktlint rule 75).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::BuiltinType;

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
        if has_expression_body(node) {
            return;
        }

        let mut cursor = node.walk();
        let mut seen_parameters = false;

        for child in node.children(&mut cursor) {
            if child.kind() == "function_value_parameters" {
                seen_parameters = true;
            } else if seen_parameters && child.kind() == ":" {
                if let Some(type_node) = find_next_type_sibling(child)
                    && is_unit_type(type_node, source)
                {
                    ctx.report(type_node, "Unnecessary \"Unit\" return type");
                }
                break;
            }
        }
    }
}

fn has_expression_body(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_body" {
            let mut body_cursor = child.walk();
            for body_child in child.children(&mut body_cursor) {
                if body_child.kind() == "=" {
                    return true;
                }
            }
        }
    }
    false
}

fn find_next_type_sibling(colon_node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = colon_node.next_sibling();
    while let Some(sibling) = current {
        let kind = sibling.kind();
        if !matches!(kind, "line_comment" | "multiline_comment" | "comment") {
            return Some(sibling);
        }
        current = sibling.next_sibling();
    }
    None
}

fn is_unit_type(type_node: tree_sitter::Node, source: &[u8]) -> bool {
    if type_node.kind() == "nullable_type" {
        return false;
    }

    let Ok(text) = type_node.utf8_text(source) else {
        return false;
    };

    BuiltinType::Unit.matches_identifier(text.trim())
}
