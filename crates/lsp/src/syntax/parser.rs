use tower_lsp::lsp_types::Position;
use tree_sitter::{Node, Parser, Point, Tree};

use crate::position::position_to_point;
use crate::signature::{find_base_indent, strip_comments_from_signature_with_base};

pub struct KotlinParser {
    parser: Parser,
}

impl Default for KotlinParser {
    fn default() -> Self {
        Self::new()
    }
}

impl KotlinParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language: tree_sitter::Language =
            unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
        parser
            .set_language(&language)
            .expect("Error loading Kotlin tree-sitter grammar");
        Self { parser }
    }

    pub fn parse(&mut self, text: &str) -> Option<Tree> {
        self.parser.parse(text, None)
    }
}

thread_local! {
    static THREAD_PARSER: std::cell::RefCell<KotlinParser> = std::cell::RefCell::new(KotlinParser::new());
}

/// Run closure with thread-local Kotlin parser
pub fn with_parser<F, R>(f: F) -> R
where
    F: FnOnce(&mut KotlinParser) -> R,
{
    THREAD_PARSER.with(|p| f(&mut p.borrow_mut()))
}

/// Parse source code using thread-local Kotlin parser
pub fn parse_source(source: &str) -> Option<Tree> {
    with_parser(|p| p.parse(source))
}

fn find_deepest_named_node<'a>(root: Node<'a>, point: Point) -> Option<Node<'a>> {
    let mut current = root.named_descendant_for_point_range(point, point)?;
    while current.kind() == "source_file" && current.named_child_count() > 0 {
        match current.named_descendant_for_point_range(point, point) {
            Some(child) if child != current => current = child,
            _ => break,
        }
    }
    Some(current)
}

#[inline]
pub fn is_identifier_node(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        "simple_identifier" | "type_identifier" | "identifier" | "interpolated_identifier"
    )
}

#[inline]
fn is_non_identifying_container(kind: &str) -> bool {
    matches!(
        kind,
        "function_value_parameters"
            | "value_arguments"
            | "type_arguments"
            | "type_parameters"
            | "statements"
            | "lambda_literal"
            | "annotated_lambda"
            | "call_suffix"
            | "function_body"
            | "class_body"
            | "block"
            | "source_file"
    )
}

/// Find the deepest named AST node at a given position
pub fn find_node_at_position<'a>(root: Node<'a>, pos: Position) -> Option<Node<'a>> {
    let point = position_to_point(pos);
    let current = find_deepest_named_node(root, point)?;

    if !is_identifier_node(&current) && pos.character > 0 {
        let prev_pos = Position {
            line: pos.line,
            character: pos.character - 1,
        };
        let prev_point = position_to_point(prev_pos);
        if let Some(prev_node) = find_deepest_named_node(root, prev_point)
            && is_identifier_node(&prev_node)
            && prev_node.end_position() == point
        {
            return Some(prev_node);
        }
    }

    Some(current)
}

/// Find the enclosing declaration node (e.g. property_declaration, parameter, class_parameter)
pub fn find_enclosing_declaration_node(node: Node) -> Node {
    let mut current = node;
    loop {
        let k = current.kind();
        if matches!(
            k,
            "property_declaration"
                | "parameter"
                | "class_parameter"
                | "function_declaration"
                | "class_declaration"
                | "object_declaration"
        ) {
            return current;
        }
        if k == "variable_declaration" {
            if let Some(parent) = current.parent()
                && matches!(
                    parent.kind(),
                    "property_declaration" | "parameter" | "class_parameter"
                )
            {
                return parent;
            }
            return current;
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if matches!(
            parent.kind(),
            "source_file" | "statements" | "function_body" | "class_body"
        ) {
            if matches!(
                k,
                "property_declaration" | "parameter" | "class_parameter" | "variable_declaration"
            ) {
                return current;
            }
            break;
        }
        current = parent;
    }
    node
}

/// Extract identifier text from node or its direct identifier child
pub fn extract_identifier_text<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    if is_identifier_node(node) {
        return source.get(node.start_byte()..node.end_byte());
    }

    if is_non_identifying_container(node.kind()) {
        return None;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ck = child.kind();
        if is_identifier_node(&child) {
            return source.get(child.start_byte()..child.end_byte());
        }
        if matches!(ck, "variable_declaration" | "type_alias")
            && let Some(name) = extract_identifier_text(&child, source)
        {
            return Some(name);
        }
    }
    None
}

/// Extract multi-line clean signature of declaration (with comments stripped)
pub fn extract_signature(node: &Node, source: &str) -> Option<String> {
    let (sig, _) = extract_signature_and_comments(node, source)?;
    Some(sig)
}

/// Extract clean signature and any stripped inline/block comments
pub fn extract_signature_and_comments(node: &Node, source: &str) -> Option<(String, Vec<String>)> {
    let mut end_byte = node.end_byte();

    if matches!(node.kind(), "parameter" | "class_parameter") {
        let mut curr_sibling = node.next_sibling();
        let mut has_equals = false;
        while let Some(sibling) = curr_sibling {
            if let Some(text) = source.get(sibling.start_byte()..sibling.end_byte()) {
                let trimmed = text.trim();
                if trimmed == "=" {
                    has_equals = true;
                    end_byte = sibling.end_byte();
                } else if has_equals {
                    if trimmed == "," || trimmed == ")" {
                        break;
                    }
                    end_byte = sibling.end_byte();
                } else {
                    break;
                }
            } else {
                break;
            }
            curr_sibling = sibling.next_sibling();
        }
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "class_body" || kind == "enum_class_body" {
                end_byte = child.start_byte();
                break;
            }
            if kind == "function_body"
                && let Some(text) = source.get(child.start_byte()..child.end_byte())
            {
                let trimmed = text.trim();
                if trimmed.starts_with('{') || trimmed.contains('\n') {
                    end_byte = child.start_byte();
                    break;
                }
            }
            if kind == "getter" || kind == "setter" {
                end_byte = child.start_byte();
                break;
            }
        }
    }

    let raw_slice = source.get(node.start_byte()..end_byte)?;
    let trimmed = raw_slice.trim().trim_end_matches('{').trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.contains('\n') && !trimmed.contains('/') {
        return Some((trimmed.to_string(), Vec::new()));
    }
    let base_indent = find_base_indent(node, source);
    let (clean_sig, comments) = strip_comments_from_signature_with_base(trimmed, base_indent);
    if clean_sig.is_empty() {
        return None;
    }
    Some((clean_sig, comments))
}

/// Check whether a declaration has private visibility modifier
pub fn is_private_declaration(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "visibility_modifier"
                    && let Some(text) = source.get(mod_child.start_byte()..mod_child.end_byte())
                    && text.trim() == "private"
                {
                    return true;
                }
            }
        } else if kind == "visibility_modifier"
            && let Some(text) = source.get(child.start_byte()..child.end_byte())
            && text.trim() == "private"
        {
            return true;
        }
    }
    false
}

/// Check whether a declaration has override modifier
pub fn has_override_modifier(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "member_modifier"
                    && let Some(text) = source.get(mod_child.start_byte()..mod_child.end_byte())
                    && text.trim() == "override"
                {
                    return true;
                }
            }
        } else if kind == "member_modifier"
            && let Some(text) = source.get(child.start_byte()..child.end_byte())
            && text.trim() == "override"
        {
            return true;
        }
    }
    false
}

/// Check if a class_parameter AST node is declared as a property (via val/var)
pub fn is_class_parameter_property(node: &Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| matches!(child.kind(), "val" | "var" | "binding_pattern_kind"))
}

/// Extract type annotation text from a parameter or property node
pub fn extract_type_annotation<'a>(node: &Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(
            kind,
            "user_type" | "type_identifier" | "nullable_type" | "function_type"
        ) && let Some(text) = source.get(child.start_byte()..child.end_byte())
        {
            let clean = text.trim();
            if !clean.is_empty() {
                return Some(clean);
            }
        }
        if kind == "variable_declaration"
            && let Some(t) = extract_type_annotation(&child, source)
        {
            return Some(t);
        }
    }
    None
}

pub use super::ast_extra::{
    extract_extension_receiver, extract_instantiated_type, extract_single_expression_body,
    strip_enclosing_parens,
};

pub use crate::signature::{
    extract_functional_input_types, extract_param_type_by_index,
    extract_return_type_from_signature, split_top_level_csv,
};
