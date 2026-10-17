//! Built-in AST rule forbidding redundant curly braces in string templates (Ktlint rule 80).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that flags redundant curly braces in simple identifier string templates (`${x}`).
pub struct StringTemplate;

impl AstRule for StringTemplate {
    fn id(&self) -> &'static str {
        "string-template"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["interpolated_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if !is_single_simple_identifier(node, source) {
            return;
        }

        let Some(start_delim) = find_interpolation_start(node) else {
            return;
        };

        let Some(end_delim) = find_interpolation_end(node) else {
            return;
        };

        let after_closing_byte = end_delim.end_byte();
        if after_closing_byte < source.len() && is_identifier_part(&source[after_closing_byte..]) {
            return;
        }

        ctx.report(start_delim, "Redundant curly braces");
    }
}

fn is_single_simple_identifier(node: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    let mut count = 0;
    let mut is_simple = false;

    for child in node.children(&mut cursor) {
        if child.is_named() {
            count += 1;
            if count == 1 && child.kind() == "simple_identifier" {
                if let Ok(text) = child.utf8_text(source)
                    && !text.starts_with('`')
                {
                    is_simple = true;
                }
            } else {
                is_simple = false;
            }
        }
    }

    count == 1 && is_simple
}

fn find_interpolation_start(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = node.prev_sibling();
    while let Some(sibling) = current {
        if sibling.kind() == "interpolation_expression_start" {
            return Some(sibling);
        }
        if sibling.is_named() || sibling.kind() == "interpolation_expression_end" {
            break;
        }
        current = sibling.prev_sibling();
    }
    None
}

fn find_interpolation_end(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut current = node.next_sibling();
    while let Some(sibling) = current {
        if sibling.kind() == "interpolation_expression_end" {
            return Some(sibling);
        }
        if sibling.is_named() || sibling.kind() == "interpolation_expression_start" {
            break;
        }
        current = sibling.next_sibling();
    }
    None
}

fn is_identifier_part(bytes: &[u8]) -> bool {
    let Some(&first) = bytes.first() else {
        return false;
    };
    if first < 0x80 {
        return first.is_ascii_alphanumeric() || first == b'_';
    }
    let char_len = match first {
        0b1100_0000..=0b1101_1111 => 2,
        0b1110_0000..=0b1110_1111 => 3,
        0b1111_0000..=0b1111_0111 => 4,
        _ => return false,
    };
    if bytes.len() < char_len {
        return false;
    }
    std::str::from_utf8(&bytes[..char_len])
        .ok()
        .and_then(|text| text.chars().next())
        .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
}
