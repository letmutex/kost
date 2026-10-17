use smol_str::SmolStr;
use tree_sitter::Node;

use super::model::{KotlinType, PrimitiveType, TypeArg, Variance};
use crate::syntax::parser::with_parser;

/// Check if a Tree-sitter node represents a Kotlin type AST node
#[inline]
pub(crate) fn is_type_node(node: &Node) -> bool {
    matches!(
        node.kind(),
        "user_type"
            | "nullable_type"
            | "function_type"
            | "parenthesized_type"
            | "not_nullable_type"
            | "dynamic"
    )
}

/// Check if a `type_modifiers` node contains the `suspend` keyword
pub(crate) fn has_suspend_modifier(node: &Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "suspend" {
            return true;
        }
    }
    false
}

/// Parse a Kotlin type signature string with known type parameters into a structured `KotlinType` using Tree-sitter
pub fn parse_type_signature_with_params(sig: &str, type_params: &[&str]) -> Option<KotlinType> {
    let clean = sig.trim();
    if clean.is_empty() || clean == "?" {
        return None;
    }

    // Fast-path: O(1) checks for single identifiers
    if !clean.contains(['<', '>', '(', ')', '?', '&', ' ', '-', '.', ':', '@']) {
        let trimmed_ident = clean.trim_matches('`');
        if type_params.contains(&trimmed_ident) {
            return Some(KotlinType::TypeParam(SmolStr::new(trimmed_ident)));
        }
        if trimmed_ident == "dynamic" {
            return Some(KotlinType::DynamicOrAny);
        }
        if let Some(prim) = PrimitiveType::from_name(trimmed_ident) {
            return Some(if prim == PrimitiveType::Nothing {
                KotlinType::Nothing
            } else {
                KotlinType::Primitive(prim)
            });
        }
        return Some(KotlinType::simple_class(trimmed_ident));
    }

    // Wrap into synthetic typealias declaration for universal Tree-sitter parsing
    let wrapped = crate::concat_str!("typealias _T = ", clean);
    let tree = with_parser(|p| p.parse(&wrapped))?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }

    let mut cursor = root.walk();
    let type_alias = root
        .children(&mut cursor)
        .find(|n| n.kind() == "type_alias")?;

    let mut is_suspend = false;
    let mut type_node = None;

    let mut alias_cursor = type_alias.walk();
    for child in type_alias.children(&mut alias_cursor) {
        match child.kind() {
            "type_modifiers" => {
                if has_suspend_modifier(&child) {
                    is_suspend = true;
                }
            }
            "=" | "typealias" | "type_identifier" => {}
            _ => {
                if is_type_node(&child) {
                    type_node = Some(child);
                }
            }
        }
    }

    let node = type_node?;
    if is_suspend && !matches!(node.kind(), "function_type" | "parenthesized_type") {
        return None;
    }

    ast_node_to_kotlin_type(&node, &wrapped, type_params, is_suspend)
}

/// Convert an AST node from Tree-sitter into a structured `KotlinType`
pub fn ast_node_to_kotlin_type(
    node: &Node,
    source: &str,
    type_params: &[&str],
    is_suspend: bool,
) -> Option<KotlinType> {
    if node.has_error() {
        return None;
    }
    let is_suspend = is_suspend
        || node
            .prev_sibling()
            .is_some_and(|prev| prev.kind() == "type_modifiers" && has_suspend_modifier(&prev));
    match node.kind() {
        "nullable_type" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "quest" && child.kind() != "?" {
                    let inner = ast_node_to_kotlin_type(&child, source, type_params, is_suspend)?;
                    return Some(inner.make_nullable());
                }
            }
            None
        }
        "not_nullable_type" => {
            let mut cursor = node.walk();
            let mut types = Vec::new();
            for child in node.children(&mut cursor) {
                if child.kind() != "&" {
                    types.push(ast_node_to_kotlin_type(&child, source, type_params, false)?);
                }
            }
            if types.is_empty() {
                None
            } else if types.len() == 1 {
                Some(types.remove(0))
            } else {
                Some(KotlinType::Intersection(types))
            }
        }
        "parenthesized_type" => {
            let mut inner_is_suspend = is_suspend;
            let mut inner_node = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "(" | ")" => {}
                    "type_modifiers" => {
                        if has_suspend_modifier(&child) {
                            inner_is_suspend = true;
                        }
                    }
                    _ => {
                        if is_type_node(&child) {
                            inner_node = Some(child);
                        }
                    }
                }
            }
            let target_node = inner_node?;
            ast_node_to_kotlin_type(&target_node, source, type_params, inner_is_suspend)
        }
        "user_type" => parse_user_type_node(node, source, type_params),
        "function_type" => parse_function_type_node(node, source, type_params, is_suspend),
        "dynamic" => Some(KotlinType::DynamicOrAny),
        _ => None,
    }
}

fn parse_user_type_node(node: &Node, source: &str, type_params: &[&str]) -> Option<KotlinType> {
    let mut name_parts: Vec<&str> = Vec::new();
    let mut type_args = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                let raw_ident = source.get(child.start_byte()..child.end_byte())?.trim();
                name_parts.push(raw_ident.trim_matches('`'));
            }
            "type_arguments" => {
                let mut args_cursor = child.walk();
                for proj in child.children(&mut args_cursor) {
                    if proj.kind() == "type_projection" {
                        type_args.push(parse_type_projection_node(&proj, source, type_params)?);
                    }
                }
            }
            _ => {}
        }
    }

    if name_parts.is_empty() {
        return None;
    }

    let full_name: SmolStr = if name_parts.len() == 1 {
        SmolStr::new(name_parts[0])
    } else {
        SmolStr::new(name_parts.join("."))
    };

    if type_args.is_empty() {
        if type_params.contains(&full_name.as_str()) {
            return Some(KotlinType::TypeParam(full_name));
        }
        if let Some(prim) = PrimitiveType::from_name(&full_name) {
            return Some(if prim == PrimitiveType::Nothing {
                KotlinType::Nothing
            } else {
                KotlinType::Primitive(prim)
            });
        }
        Some(KotlinType::simple_class(full_name))
    } else {
        Some(KotlinType::class(full_name, type_args))
    }
}

fn parse_type_projection_node(node: &Node, source: &str, type_params: &[&str]) -> Option<TypeArg> {
    let mut variance = Variance::Invariant;
    let mut type_child = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "*" => return Some(TypeArg::STAR),
            "type_projection_modifiers" => {
                variance = extract_projection_variance(&child, source);
            }
            _ => {
                if is_type_node(&child) {
                    type_child = Some(child);
                }
            }
        }
    }

    let target_node = type_child?;
    let type_ref = ast_node_to_kotlin_type(&target_node, source, type_params, false)?;
    Some(TypeArg { variance, type_ref })
}

fn extract_projection_variance(node: &Node, source: &str) -> Variance {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variance_modifier"
            && let Some(text) = source.get(child.start_byte()..child.end_byte())
        {
            if text == "out" {
                return Variance::Out;
            }
            if text == "in" {
                return Variance::In;
            }
        }
    }
    Variance::Invariant
}

fn parse_function_type_node(
    node: &Node,
    source: &str,
    type_params: &[&str],
    mut is_suspend: bool,
) -> Option<KotlinType> {
    let mut receiver = None;
    let mut params = Vec::new();
    let mut return_type = None;
    let mut is_after_arrow = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "suspend" => {
                is_suspend = true;
            }
            "receiver_type" => {
                let mut receiver_cursor = child.walk();
                for rchild in child.children(&mut receiver_cursor) {
                    if is_type_node(&rchild) {
                        receiver = ast_node_to_kotlin_type(&rchild, source, type_params, false);
                        break;
                    }
                }
            }
            "function_type_parameters" => {
                let mut param_cursor = child.walk();
                for pchild in child.children(&mut param_cursor) {
                    if let Some(param_ty) = extract_func_param_type(&pchild, source, type_params) {
                        params.push(param_ty);
                    }
                }
            }
            "->" => {
                is_after_arrow = true;
            }
            _ => {
                if is_after_arrow && is_type_node(&child) {
                    return_type = ast_node_to_kotlin_type(&child, source, type_params, false);
                }
            }
        }
    }

    let ret = return_type?;
    Some(KotlinType::function(receiver, params, ret, is_suspend))
}

fn extract_func_param_type(
    param_node: &Node,
    source: &str,
    type_params: &[&str],
) -> Option<KotlinType> {
    if param_node.kind() == "parameter" {
        let mut param_cursor = param_node.walk();
        let mut is_suspend = false;
        let mut type_child = None;
        for arg in param_node.children(&mut param_cursor) {
            if arg.kind() == "type_modifiers" && has_suspend_modifier(&arg) {
                is_suspend = true;
            } else if is_type_node(&arg) {
                type_child = Some(arg);
            }
        }
        let target = type_child?;
        ast_node_to_kotlin_type(&target, source, type_params, is_suspend)
    } else if is_type_node(param_node) {
        ast_node_to_kotlin_type(param_node, source, type_params, false)
    } else {
        None
    }
}
