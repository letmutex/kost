use tree_sitter::Node;

use super::ast_eval::AstEvalContext;
use super::types::KotlinType;

/// Infer type for terminal literal AST nodes according to tree-sitter-kotlin grammar
pub fn infer_literal(ctx: &AstEvalContext, node: Node) -> Option<KotlinType> {
    let text = ctx.node_text(node)?;
    infer_literal_kind_and_text(node.kind(), text)
}

/// Infer literal type by node kind and source text
pub fn infer_literal_kind_and_text(kind: &str, text: &str) -> Option<KotlinType> {
    match kind {
        "integer_literal" => {
            if text.ends_with('L') || text.ends_with('l') {
                Some(KotlinType::LONG)
            } else if text.ends_with('u') || text.ends_with('U') {
                Some(KotlinType::simple_class("UInt"))
            } else {
                Some(KotlinType::INT)
            }
        }
        "long_literal" => Some(KotlinType::LONG),
        "hex_literal" | "bin_literal" => {
            if text.ends_with('L') || text.ends_with('l') {
                Some(KotlinType::LONG)
            } else if text.ends_with('u') || text.ends_with('U') {
                Some(KotlinType::simple_class("UInt"))
            } else {
                Some(KotlinType::INT)
            }
        }
        "unsigned_literal" => {
            if text.ends_with('L') || text.ends_with('l') {
                Some(KotlinType::simple_class("ULong"))
            } else {
                Some(KotlinType::simple_class("UInt"))
            }
        }
        "real_literal" | "float_literal" => {
            if text.ends_with('F') || text.ends_with('f') {
                Some(KotlinType::FLOAT)
            } else {
                Some(KotlinType::DOUBLE)
            }
        }
        "boolean_literal" => Some(KotlinType::BOOLEAN),
        "character_literal" => Some(KotlinType::CHAR),
        "string_literal" | "line_string_expression" | "multi_line_string_expression" => {
            Some(KotlinType::STRING)
        }
        "null" | "null_literal" => Some(KotlinType::NOTHING.make_nullable()),
        _ => None,
    }
}

/// Check if an AST node kind is a terminal literal
pub fn is_literal_kind(kind: &str) -> bool {
    matches!(
        kind,
        "integer_literal"
            | "long_literal"
            | "hex_literal"
            | "bin_literal"
            | "unsigned_literal"
            | "real_literal"
            | "float_literal"
            | "boolean_literal"
            | "character_literal"
            | "string_literal"
            | "line_string_expression"
            | "multi_line_string_expression"
            | "null"
            | "null_literal"
    )
}
