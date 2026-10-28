//! Built-in AST rule enforcing property naming conventions (Ktlint rule 77).

use super::helpers::{
    find_property_identifier, has_modifier_named, is_lower_camel_case, next_code_sibling,
};
use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{is_kotlin_keyword, node_kind};

/// Rule enforcing lowerCamelCase naming for properties and SCREAMING_SNAKE_CASE for constants.
pub struct PropertyNaming;

impl AstRule for PropertyNaming {
    fn id(&self) -> &'static str {
        "property-naming"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["property_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(identifier_node) = find_property_identifier(node) else {
            return;
        };

        let Ok(text) = identifier_node.utf8_text(source) else {
            return;
        };

        if text.starts_with('`') && text.ends_with('`') && text.len() >= 2 {
            let inner = &text[1..text.len() - 1];
            if is_kotlin_keyword(inner) {
                return;
            }
        }

        if has_const_modifier(node, source) {
            check_const_property(identifier_node, text, ctx);
        } else if text.starts_with('_')
            || is_lower_camel_case(text)
            || has_custom_getter(node)
            || is_top_level_val(node, ctx)
            || is_object_val(node, ctx)
        {
            // Valid lowerCamelCase or cannot reliably determine mutability
        } else {
            check_non_const_property(identifier_node, text, ctx);
        }
    }
}

fn has_const_modifier(node: tree_sitter::Node, source: &[u8]) -> bool {
    has_modifier_named(node, "const", source)
}

fn has_custom_getter(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if node_kind(child) == "getter" {
            return true;
        }
    }
    if let Some(next) = next_code_sibling(node)
        && node_kind(next) == "getter"
    {
        return true;
    }
    false
}

fn is_top_level_val(node: tree_sitter::Node, ctx: &LintCtx) -> bool {
    let Some(parent) = ctx.parent() else {
        return false;
    };
    if node_kind(parent) != "source_file" {
        return false;
    }
    node_has_val(node)
}

fn is_object_val(node: tree_sitter::Node, ctx: &LintCtx) -> bool {
    let Some(class_body) = ctx.parent() else {
        return false;
    };
    if node_kind(class_body) != "class_body" {
        return false;
    }
    let Some(object_decl) = ctx.ancestor(2) else {
        return false;
    };
    if !matches!(
        node_kind(object_decl),
        "object_declaration" | "companion_object"
    ) {
        return false;
    }
    node_has_val(node)
}

fn node_has_val(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = node_kind(child);
        if kind == "val"
            || (kind == "binding_pattern_kind"
                && child.child(0).is_some_and(|k| node_kind(k) == "val"))
        {
            return true;
        }
    }
    false
}

fn check_const_property(identifier_node: tree_sitter::Node, text: &str, ctx: &mut LintCtx) {
    if text == "serialVersionUID" || is_screaming_snake_case(text) {
        return;
    }
    ctx.report(
        identifier_node,
        "Property name should use the screaming snake case notation when the value can not be changed",
    );
}

fn check_non_const_property(identifier_node: tree_sitter::Node, text: &str, ctx: &mut LintCtx) {
    if text.starts_with('_') || is_lower_camel_case(text) {
        return;
    }
    ctx.report(
        identifier_node,
        "Property name should start with a lowercase letter and use camel case",
    );
}

fn is_screaming_snake_case(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|character| {
        character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
    })
}
