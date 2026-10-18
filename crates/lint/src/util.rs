//! Utility functions and string concatenation macros.

use smol_str::SmolStr;

/// Helper function to convert any string-like reference into `&str`.
#[inline(always)]
pub fn as_str<T: ?Sized + AsRef<str>>(reference: &T) -> &str {
    reference.as_ref()
}

/// Concatenates multiple string slices into a heap `String`.
pub fn concat_str_slice(parts: &[&str]) -> String {
    let total_length: usize = parts.iter().map(|string_slice| string_slice.len()).sum();
    let mut result = String::with_capacity(total_length);
    for part in parts {
        result.push_str(part);
    }
    result
}

/// Concatenates multiple string slices into an inline `SmolStr` when possible.
pub fn concat_smol_str_slice(parts: &[&str]) -> SmolStr {
    let total_length: usize = parts.iter().map(|string_slice| string_slice.len()).sum();
    if total_length <= 22 {
        let mut buffer = [0u8; 22];
        let mut offset = 0;
        for part in parts {
            let bytes = part.as_bytes();
            buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
            offset += bytes.len();
        }
        let string_slice = match std::str::from_utf8(&buffer[..total_length]) {
            Ok(valid) => valid,
            Err(_) => unreachable!("Concatenation of valid UTF-8 slices is always valid UTF-8"),
        };
        SmolStr::new(string_slice)
    } else {
        let mut string = String::with_capacity(total_length);
        for part in parts {
            string.push_str(part);
        }
        SmolStr::from(string)
    }
}

/// Concatenates string-like arguments into a `String` with a single allocation.
#[macro_export]
macro_rules! concat_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Concatenates string-like arguments into a `SmolStr`.
#[macro_export]
macro_rules! concat_smol_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_smol_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Finds a child identifier or variable identifier for an AST node.
pub fn find_child_identifier(node: tree_sitter::Node, source: &[u8]) -> Option<SmolStr> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "simple_identifier" | "type_identifier")
            && let Ok(text) = child.utf8_text(source)
        {
            return Some(SmolStr::new(text));
        }
        if child.kind() == "variable_declaration" {
            let mut var_cursor = child.walk();
            for var_child in child.children(&mut var_cursor) {
                if matches!(var_child.kind(), "simple_identifier" | "type_identifier")
                    && let Ok(text) = var_child.utf8_text(source)
                {
                    return Some(SmolStr::new(text));
                }
            }
        }
    }
    None
}

/// Counts the number of parameters declared by a function node.
pub fn count_function_parameters(node: tree_sitter::Node) -> i64 {
    let mut count = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            let mut params_cursor = child.walk();
            for param in child.children(&mut params_cursor) {
                if param.kind() == "parameter" {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Checks whether an AST declaration node contains any annotations.
pub fn has_declaration_annotations(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if matches!(
                    mod_child.kind(),
                    "annotation" | "single_annotation" | "file_annotation"
                ) {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns the nearest preceding sibling that is either named or a comment.
pub fn prev_meaningful_sibling(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        if sibling.is_named()
            || sibling.kind() == "line_comment"
            || sibling.kind() == "multiline_comment"
        {
            return Some(sibling);
        }
        current = sibling.prev_sibling();
    }
    None
}

/// Returns the nearest preceding declaration sibling.
pub fn prev_declaration_sibling(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = node.prev_named_sibling();
    while let Some(sibling) = current {
        if matches!(
            sibling.kind(),
            "class_declaration"
                | "object_declaration"
                | "function_declaration"
                | "property_declaration"
                | "secondary_constructor"
                | "anonymous_initializer"
                | "type_alias"
        ) {
            return Some(sibling);
        }
        current = sibling.prev_named_sibling();
    }
    None
}

/// Checks whether there is at least one blank line between two declaration siblings.
pub fn has_blank_line_between_decls(
    prev_decl: tree_sitter::Node,
    curr_decl: tree_sitter::Node,
) -> bool {
    let mut last_end_row = prev_decl.end_position().row;
    let mut current = prev_decl.next_sibling();
    while let Some(sibling) = current {
        if sibling == curr_decl {
            break;
        }
        if sibling.start_position().row > last_end_row + 1 {
            return true;
        }
        last_end_row = last_end_row.max(sibling.end_position().row);
        current = sibling.next_sibling();
    }
    curr_decl.start_position().row > last_end_row + 1
}

/// Whitespace preceding an AST token on the same line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceBefore {
    StartOfLine,
    Spaces(usize),
}

/// Whitespace following an AST token on the same line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceAfter {
    EndOfLine { trailing_spaces: usize },
    Spaces(usize),
}

/// Returns horizontal whitespace preceding a node on the same line.
pub fn spaces_before(node: tree_sitter::Node, source: &[u8]) -> WhitespaceBefore {
    let start = node.start_byte();
    if start == 0 {
        return WhitespaceBefore::StartOfLine;
    }
    let mut count = 0;
    let mut i = start;
    while i > 0 {
        i -= 1;
        match source[i] {
            b' ' | b'\t' => count += 1,
            b'\n' | b'\r' => return WhitespaceBefore::StartOfLine,
            _ => return WhitespaceBefore::Spaces(count),
        }
    }
    WhitespaceBefore::StartOfLine
}

/// Returns horizontal whitespace following a node on the same line.
pub fn spaces_after(node: tree_sitter::Node, source: &[u8]) -> WhitespaceAfter {
    let end = node.end_byte();
    let mut count = 0;
    let mut i = end;
    while i < source.len() {
        match source[i] {
            b' ' | b'\t' => {
                count += 1;
                i += 1;
            }
            b'\n' | b'\r' => {
                return WhitespaceAfter::EndOfLine {
                    trailing_spaces: count,
                };
            }
            _ => return WhitespaceAfter::Spaces(count),
        }
    }
    WhitespaceAfter::EndOfLine {
        trailing_spaces: count,
    }
}

/// Checks whether a control structure body or block is enclosed in curly braces `{ ... }`.
pub fn is_braced_body(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "{" {
            return true;
        }
    }
    false
}

/// Built-in Kotlin type representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    Unit,
    Any,
}

impl BuiltinType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Any => "Any",
        }
    }

    pub fn matches_identifier(self, name: &str) -> bool {
        match self {
            Self::Unit => name == "Unit" || name == "kotlin.Unit",
            Self::Any => name == "Any" || name == "kotlin.Any",
        }
    }
}

/// Finds an immediate comma sibling between `start_node` and `end_node`.
pub fn find_comma_between<'a>(
    start_node: tree_sitter::Node<'a>,
    end_node: tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let mut current = start_node.next_sibling();
    while let Some(sibling) = current {
        if sibling == end_node {
            break;
        }
        if sibling.kind() == "," {
            return Some(sibling);
        }
        current = sibling.next_sibling();
    }
    None
}

/// Returns true if byte slice is a KDoc comment (starts with `/**`, not `/***`, ends with `*/`, and not `/**/`).
pub fn is_kdoc_bytes(bytes: &[u8]) -> bool {
    bytes.starts_with(b"/**")
        && !bytes.starts_with(b"/***")
        && bytes.ends_with(b"*/")
        && bytes != b"/**/"
}

/// Returns true if text is a KDoc comment (starts with `/**`, not `/***`, ends with `*/`, and not `/**/`).
pub fn is_kdoc(text: &str) -> bool {
    is_kdoc_bytes(text.as_bytes())
}

/// Checks whether an identifier is a reserved Kotlin keyword or soft keyword.
#[rustfmt::skip]
pub fn is_kotlin_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break" | "class" | "continue" | "do" | "else" | "false" | "for" | "fun" | "if"
            | "in" | "interface" | "is" | "null" | "object" | "package" | "return" | "super"
            | "this" | "throw" | "true" | "try" | "typealias" | "typeof" | "val" | "var"
            | "when" | "while" | "by" | "catch" | "constructor" | "delegate" | "dynamic"
            | "field" | "file" | "finally" | "get" | "import" | "init" | "param" | "property"
            | "receiver" | "set" | "setparam" | "value" | "where" | "actual" | "abstract"
            | "annotation" | "companion" | "const" | "crossinline" | "data" | "enum" | "expect"
            | "external" | "final" | "infix" | "inline" | "inner" | "internal" | "lateinit"
            | "noinline" | "open" | "operator" | "out" | "override" | "private" | "protected"
            | "public" | "reified" | "sealed" | "suspend" | "tailrec" | "vararg"
    )
}

/// Computes the column indentation of the line starting at `node_start_byte`.
pub fn compute_line_indent(source: &[u8], node_start_byte: usize) -> i64 {
    let mut line_start = node_start_byte;
    while line_start > 0 && source[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    let mut indent: i64 = 0;
    for &byte in &source[line_start..] {
        match byte {
            b' ' => indent += 1,
            b'\t' => indent += 4,
            b'\r' => {}
            _ => break,
        }
    }
    indent
}

/// Recursively checks whether an annotation node contains argument lists.
pub fn annotation_has_arguments(node: tree_sitter::Node) -> bool {
    if matches!(node.kind(), "value_arguments" | "value_argument_list") {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if annotation_has_arguments(child) {
            return true;
        }
    }
    false
}
