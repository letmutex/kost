//! Built-in AST rule enforcing backing property naming conventions (Ktlint rule 78).

use super::helpers::{find_function_identifier, find_property_identifier, has_modifier_named};
use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule enforcing conventions for backing properties (prefixed with underscore).
pub struct BackingPropertyNaming;

impl AstRule for BackingPropertyNaming {
    fn id(&self) -> &'static str {
        "backing-property-naming"
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

        if !text.starts_with('_') || text == "_" || has_override_modifier(node, source) {
            return;
        }

        if !is_valid_backing_property_name(text) {
            ctx.report(
                identifier_node,
                "Backing property should start with underscore followed by lower camel case",
            );
            return;
        }

        if !has_private_modifier(node, source) && !is_in_private_companion_object(ctx, source) {
            ctx.report(
                identifier_node,
                "Backing property not allowed when 'private' modifier is missing",
            );
            return;
        }

        let correlated_name = &text[1..];
        let correlated_member = find_correlated_member(node, correlated_name, source, ctx);

        let Some(correlated_node) = correlated_member else {
            ctx.report(
                identifier_node,
                "Backing property is only allowed when a matching property or function exists",
            );
            return;
        };

        if has_non_public_visibility(correlated_node, source) {
            ctx.report(
                identifier_node,
                "Backing property is only allowed when the matching property or function is public",
            );
        }
    }
}

fn is_valid_backing_property_name(text: &str) -> bool {
    let mut chars = text.chars();
    if chars.next() != Some('_') {
        return false;
    }
    match chars.next() {
        Some(second) if second.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|character| character.is_ascii_alphanumeric())
}

fn has_override_modifier(node: tree_sitter::Node, source: &[u8]) -> bool {
    has_modifier_named(node, "override", source)
}

fn has_private_modifier(node: tree_sitter::Node, source: &[u8]) -> bool {
    has_modifier_named(node, "private", source)
}

fn is_companion_object(decl: tree_sitter::Node, source: &[u8]) -> bool {
    decl.kind() == "companion_object"
        || (decl.kind() == "object_declaration" && has_modifier_named(decl, "companion", source))
}

fn is_in_private_companion_object(ctx: &LintCtx, source: &[u8]) -> bool {
    let Some(class_body) = ctx.parent() else {
        return false;
    };
    if class_body.kind() != "class_body" {
        return false;
    }
    let Some(object_decl) = ctx.ancestor(2) else {
        return false;
    };
    if is_companion_object(object_decl, source) {
        return has_modifier_named(object_decl, "private", source);
    }
    false
}

fn has_non_public_visibility(node: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if let Ok(text) = mod_child.utf8_text(source)
                    && matches!(text, "private" | "protected" | "internal")
                {
                    return true;
                }
            }
        }
    }
    false
}

fn find_correlated_member<'a>(
    node: tree_sitter::Node<'a>,
    correlated_name: &str,
    source: &[u8],
    ctx: &LintCtx<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let class_body = ctx.parent()?;
    if class_body.kind() != "class_body" {
        return None;
    }

    let func_name = common::concat_str!("get", capitalize_first(correlated_name));

    if let Some(found) =
        search_body_for_correlated(class_body, node, correlated_name, &func_name, source)
    {
        return Some(found);
    }

    if let Some(obj_decl) = ctx.ancestor(2)
        && is_companion_object(obj_decl, source)
        && let Some(outer_body) = ctx.ancestor(3)
        && outer_body.kind() == "class_body"
        && let Some(found) =
            search_body_for_correlated(outer_body, node, correlated_name, &func_name, source)
    {
        return Some(found);
    }

    let mut cursor = class_body.walk();
    for child in class_body.children(&mut cursor) {
        if is_companion_object(child, source)
            && let Some(companion_body) = find_class_body(child)
            && let Some(found) = search_body_for_correlated(
                companion_body,
                node,
                correlated_name,
                &func_name,
                source,
            )
        {
            return Some(found);
        }
    }

    None
}

fn find_class_body(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|c| c.kind() == "class_body")
}

fn search_body_for_correlated<'a>(
    body: tree_sitter::Node<'a>,
    origin: tree_sitter::Node<'a>,
    prop_name: &str,
    func_name: &str,
    source: &[u8],
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child == origin {
            continue;
        }
        if child.kind() == "property_declaration"
            && let Some(ident) = find_property_identifier(child)
            && ident.utf8_text(source) == Ok(prop_name)
        {
            return Some(child);
        }
        if child.kind() == "function_declaration"
            && let Some(ident) = find_function_identifier(child)
            && ident.utf8_text(source) == Ok(func_name)
            && has_zero_parameters(child)
        {
            return Some(child);
        }
    }
    None
}

fn has_zero_parameters(fn_node: tree_sitter::Node) -> bool {
    let mut cursor = fn_node.walk();
    for child in fn_node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            let mut param_cursor = child.walk();
            return !child
                .children(&mut param_cursor)
                .any(|c| c.kind() == "parameter");
        }
    }
    true
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => common::concat_str!(first.to_uppercase().to_string(), chars.as_str()),
    }
}
