//! Built-in AST rule enforcing enum entry name casing (Ktlint rule 79).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule enforcing uppercase underscore or UpperCamelCase for enum entry names.
pub struct EnumEntryNameCase;

impl AstRule for EnumEntryNameCase {
    fn id(&self) -> &'static str {
        "enum-entry-name-case"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["enum_entry"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(identifier_node) = find_enum_entry_identifier(node) else {
            return;
        };

        let Ok(raw_text) = identifier_node.utf8_text(source) else {
            return;
        };

        let text = if raw_text.starts_with('`') && raw_text.ends_with('`') && raw_text.len() >= 2 {
            &raw_text[1..raw_text.len() - 1]
        } else {
            raw_text
        };

        if is_valid_enum_entry_name(text) {
            return;
        }

        ctx.report(
            identifier_node,
            "Enum entry name should be uppercase underscore-separated names like \"ENUM_ENTRY\" or upper camel-case like \"EnumEntry\"",
        );
    }
}

fn find_enum_entry_identifier(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|&child| child.kind() == "simple_identifier")
}

fn is_valid_enum_entry_name(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }

    let is_screaming_snake = chars.clone().all(|character| {
        character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
    });

    let is_upper_camel = chars.all(|character| character.is_ascii_alphanumeric());

    is_screaming_snake || is_upper_camel
}
