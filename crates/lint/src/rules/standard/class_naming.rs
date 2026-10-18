//! Built-in AST rule enforcing class naming conventions (Ktlint rule 75).

use super::helpers::find_root_node;
use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_kotlin_keyword;

/// Rule enforcing PascalCase naming for classes and objects.
pub struct ClassNaming;

impl AstRule for ClassNaming {
    fn id(&self) -> &'static str {
        "class-naming"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(identifier_node) = find_declaration_identifier(node) else {
            return;
        };

        let Ok(text) = identifier_node.utf8_text(source) else {
            return;
        };

        if is_valid_class_name(text) {
            return;
        }

        if text.starts_with('`') && text.ends_with('`') && text.len() >= 2 {
            let inner = &text[1..text.len() - 1];
            if is_kotlin_keyword(inner) {
                return;
            }
            if has_junit_jupiter_import(find_root_node(node), source) {
                return;
            }
        }

        ctx.report(
            identifier_node,
            "Class or object name should start with an uppercase letter and use camel case",
        );
    }
}

fn find_declaration_identifier(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "type_identifier" | "simple_identifier") {
            return Some(child);
        }
    }
    None
}

fn is_valid_class_name(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|character| character.is_ascii_alphanumeric())
}

fn has_junit_jupiter_import(root: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "import_list" {
            let mut list_cursor = child.walk();
            for header in child.children(&mut list_cursor) {
                if header.kind() == "import_header" {
                    let mut header_cursor = header.walk();
                    for item in header.children(&mut header_cursor) {
                        if item.kind() == "identifier"
                            && let Ok(text) = item.utf8_text(source)
                            && is_junit_jupiter_package_path(text)
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn is_junit_jupiter_package_path(path: &str) -> bool {
    const JUPITER: &str = "org.junit.jupiter.api";
    path == JUPITER || (path.starts_with(JUPITER) && path[JUPITER.len()..].starts_with('.'))
}
