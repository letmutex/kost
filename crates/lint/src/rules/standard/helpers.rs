//! Shared AST navigation and inspection utilities for Kotlin lint rules.

/// Finds the root `source_file` node by ascending the parent hierarchy.
pub fn find_root_node(mut node: tree_sitter::Node) -> tree_sitter::Node {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}

/// Finds the identifier node of a `property_declaration`.
pub fn find_property_identifier(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "simple_identifier" {
            return Some(child);
        }
        if child.kind() == "variable_declaration" {
            let mut sub_cursor = child.walk();
            for sub_child in child.children(&mut sub_cursor) {
                if sub_child.kind() == "simple_identifier" {
                    return Some(sub_child);
                }
            }
        }
    }
    None
}

/// Finds the identifier node of a `function_declaration`.
pub fn find_function_identifier(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "simple_identifier" | "type_identifier") {
            return Some(child);
        }
    }
    None
}

/// Checks whether a declaration has a specific modifier keyword.
pub fn has_modifier_named(node: tree_sitter::Node, expected: &str, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == expected || mod_child.utf8_text(source) == Ok(expected) {
                    return true;
                }
            }
        }
    }
    false
}

/// Finds the next non-comment code sibling of a node.
pub fn next_code_sibling(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut curr = node.next_named_sibling();
    while let Some(sibling) = curr {
        if !matches!(
            sibling.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            return Some(sibling);
        }
        curr = sibling.next_named_sibling();
    }
    None
}

/// Finds the previous non-comment code sibling of a node.
pub fn prev_code_sibling(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut curr = node.prev_named_sibling();
    while let Some(sibling) = curr {
        if !matches!(
            sibling.kind(),
            "comment" | "line_comment" | "multiline_comment"
        ) {
            return Some(sibling);
        }
        curr = sibling.prev_named_sibling();
    }
    None
}

/// Checks whether an identifier string is formatted in lowerCamelCase (starts with lowercase ASCII, alphanumeric only).
pub fn is_lower_camel_case(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|character| character.is_ascii_alphanumeric())
}

/// Fast blank-line check between declaration siblings using parent cursor when needed.
pub fn has_blank_line_between_decls_with_context(
    prev_decl: tree_sitter::Node,
    curr_decl: tree_sitter::Node,
    prev_sibling: Option<tree_sitter::Node>,
    parent: Option<tree_sitter::Node>,
) -> bool {
    if curr_decl.start_position().row <= prev_decl.end_position().row + 1 {
        return false;
    }
    if prev_sibling == Some(prev_decl) {
        return true;
    }
    let Some(parent_node) = parent else {
        return crate::util::has_blank_line_between_decls(prev_decl, curr_decl);
    };
    let (prev_end, curr_start) = (prev_decl.end_byte(), curr_decl.start_byte());
    let mut last_end_row = prev_decl.end_position().row;
    let mut cursor = parent_node.walk();
    for sibling in parent_node.children(&mut cursor) {
        if sibling.start_byte() < prev_end || sibling == prev_decl {
            continue;
        }
        if sibling.start_byte() >= curr_start || sibling == curr_decl {
            break;
        }
        if sibling.start_position().row > last_end_row + 1 {
            return true;
        }
        last_end_row = last_end_row.max(sibling.end_position().row);
    }
    curr_decl.start_position().row > last_end_row + 1
}
