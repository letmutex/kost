use std::collections::HashSet;
use tower_lsp::lsp_types::{Range, SymbolKind};
use tree_sitter::Node;

use crate::parser::{extract_identifier_text, extract_signature};
use crate::position::node_to_range;

#[derive(Debug, Clone)]
pub struct ScopeSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub signature: Option<String>,
    pub is_function: bool,
    pub is_param: bool,
}

use super::scope_enclosing::{check_declaration_node, check_param_or_constructor_child};
pub use super::scope_enclosing::{collect_enclosing_classes, find_enclosing_class_name};

pub fn find_local_variable_or_param<'a>(
    scope_node: Node<'a>,
    name: &str,
    source: &'a str,
) -> Option<Node<'a>> {
    find_local_variable_or_param_before(scope_node, name, source, usize::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingRole {
    Value,
    DirectCall,
    CallableRef,
}

pub fn find_local_variable_or_param_before<'a>(
    scope_node: Node<'a>,
    name: &str,
    source: &'a str,
    before_byte: usize,
) -> Option<Node<'a>> {
    find_local_variable_or_param_before_with_role(
        scope_node,
        name,
        source,
        before_byte,
        BindingRole::Value,
    )
}

pub fn find_declarations_in_scope<'a>(
    scope: Node<'a>,
    name: &str,
    source: &'a str,
    before_byte: usize,
    role: BindingRole,
) -> Option<Node<'a>> {
    let sk = scope.kind();
    let mut cursor = scope.walk();
    for child in scope.children(&mut cursor) {
        if let Some(found) =
            check_param_or_constructor_child(child, name, source, before_byte, role)
        {
            return Some(found);
        } else if sk == "statements" {
            if child.start_byte() <= before_byte
                && let Some(found) = check_declaration_node(child, name, source, before_byte, role)
            {
                return Some(found);
            }
        } else if sk == "class_body" {
            if let Some(found) = check_declaration_node(child, name, source, usize::MAX, role) {
                return Some(found);
            }
        } else if sk == "catch_block" {
            if let Some(found) = check_declaration_node(child, name, source, before_byte, role) {
                return Some(found);
            }
        } else if matches!(
            child.kind(),
            "parameter"
                | "class_parameter"
                | "variable_declaration"
                | "function_declaration"
                | "property_declaration"
                | "multi_variable_declaration"
        ) && let Some(found) =
            check_declaration_node(child, name, source, before_byte, role)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_local_variable_or_param_before_with_role<'a>(
    scope_node: Node<'a>,
    name: &str,
    source: &'a str,
    before_byte: usize,
    role: BindingRole,
) -> Option<Node<'a>> {
    find_declarations_in_scope(scope_node, name, source, before_byte, role)
}

pub fn find_lexical_binding_node_before<'a>(
    node: Node<'a>,
    symbol_name: &str,
    source: &'a str,
    before_byte: usize,
    role: BindingRole,
) -> Option<Node<'a>> {
    let mut current = Some(node);
    while let Some(parent) = current {
        let is_source_file = parent.kind() == "source_file";
        let check_byte = if is_source_file {
            usize::MAX
        } else {
            before_byte
        };
        if let Some(found) =
            find_declarations_in_scope(parent, symbol_name, source, check_byte, role)
        {
            return Some(found);
        } else if role == BindingRole::DirectCall
            && let Some(val_decl) = find_declarations_in_scope(
                parent,
                symbol_name,
                source,
                check_byte,
                BindingRole::Value,
            )
        {
            return Some(val_decl);
        }
        if is_source_file {
            break;
        }
        current = parent.parent();
    }
    None
}

pub fn find_lexical_binding_node<'a>(
    node: Node<'a>,
    symbol_name: &str,
    source: &'a str,
    role: BindingRole,
) -> Option<Node<'a>> {
    let before_byte = if matches!(
        node.kind(),
        "source_file" | "class_body" | "function_body" | "statements" | "block"
    ) {
        node.end_byte()
    } else {
        node.start_byte()
    };
    find_lexical_binding_node_before(node, symbol_name, source, before_byte, role)
}

pub fn collect_local_scope_symbols(
    node: Node,
    source: &str,
    partial_lower: &str,
    before_byte: usize,
    seen_labels: &mut HashSet<String>,
    symbols: &mut Vec<ScopeSymbol>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let k = child.kind();
        let is_catch_param = node.kind() == "catch_block" && k == "simple_identifier";
        if matches!(
            k,
            "property_declaration"
                | "parameter"
                | "class_parameter"
                | "variable_declaration"
                | "function_declaration"
        ) || is_catch_param
        {
            let is_param = matches!(k, "parameter" | "class_parameter") || is_catch_param;
            if (is_param || child.start_byte() < before_byte)
                && let Some(name) = extract_identifier_text(&child, source)
                && (partial_lower.is_empty() || name.to_lowercase().contains(partial_lower))
                && seen_labels.insert(name.to_string())
            {
                let is_function = k == "function_declaration";
                let kind = if is_param {
                    SymbolKind::VARIABLE
                } else if is_function {
                    SymbolKind::FUNCTION
                } else {
                    SymbolKind::PROPERTY
                };
                symbols.push(ScopeSymbol {
                    name: name.to_string(),
                    kind,
                    range: node_to_range(&child),
                    signature: extract_signature(&child, source),
                    is_function,
                    is_param,
                });
            }
        }
        if matches!(
            k,
            "function_value_parameters"
                | "class_parameters"
                | "primary_constructor"
                | "lambda_parameters"
                | "statements"
                | "multi_variable_declaration"
                | "destructuring_declaration"
                | "property_declaration"
                | "when_subject"
        ) {
            collect_local_scope_symbols(
                child,
                source,
                partial_lower,
                before_byte,
                seen_labels,
                symbols,
            );
        }
    }
}

pub use super::scope_enclosing::find_parameter;
