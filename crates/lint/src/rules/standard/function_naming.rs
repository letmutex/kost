//! Built-in AST rule enforcing function naming conventions (Ktlint rule 76).

use super::helpers::{find_function_identifier, find_root_node};
use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_kotlin_keyword;

/// Rule enforcing camelCase naming for functions.
pub struct FunctionNaming;

impl AstRule for FunctionNaming {
    fn id(&self) -> &'static str {
        "function-naming"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["function_declaration"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(identifier_node) = find_function_identifier(node) else {
            return;
        };

        let Ok(text) = identifier_node.utf8_text(source) else {
            return;
        };

        if is_valid_function_name(text) {
            return;
        }

        if has_override_modifier(node, source)
            || is_annotated_with_ignored_annotation(node, source, ctx)
        {
            return;
        }

        if is_factory_method(node, text, source) {
            return;
        }

        let is_test_file = match ctx.has_test_import {
            Some(cached) => cached,
            None => {
                let res = has_test_library_import(find_root_node(node), source);
                ctx.has_test_import = Some(res);
                res
            }
        };

        if is_test_file && is_valid_test_function_name(text) {
            return;
        }

        if text.starts_with('`') && text.ends_with('`') && text.len() >= 2 {
            let inner = &text[1..text.len() - 1];
            if is_kotlin_keyword(inner) {
                return;
            }
        }

        ctx.report(
            identifier_node,
            "Function name should start with a lowercase letter (except factory methods) and use camel case",
        );
    }
}

fn has_override_modifier(node: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "override"
                    || (mod_child.kind() == "member_modifier"
                        && mod_child.utf8_text(source) == Ok("override"))
                {
                    return true;
                }
            }
        }
    }
    false
}

fn is_annotated_with_ignored_annotation(
    node: tree_sitter::Node,
    source: &[u8],
    ctx: &LintCtx,
) -> bool {
    let mut ignored: smallvec::SmallVec<[&str; 4]> = smallvec::smallvec!["Composable"];

    if let Some(config) = ctx.editorconfig
        && let Some(val) = config
            .raw_properties
            .get("ktlint_function_naming_ignore_when_annotated_with")
            .or_else(|| {
                config
                    .raw_properties
                    .get("ktlint_standard_function_naming_ignore_when_annotated_with")
            })
            .or_else(|| {
                config
                    .raw_properties
                    .get("ktlint_standard_function-naming_ignore_when_annotated_with")
            })
    {
        for part in val.as_str().split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() && !ignored.contains(&trimmed) {
                ignored.push(trimmed);
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if matches!(mod_child.kind(), "annotation" | "single_annotation") {
                    for &name in &ignored {
                        if has_annotation_name(mod_child, name, source) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn has_annotation_name(annotation: tree_sitter::Node, expected: &str, source: &[u8]) -> bool {
    let mut cursor = annotation.walk();
    for child in annotation.children(&mut cursor) {
        if matches!(child.kind(), "type_identifier" | "simple_identifier")
            && child.utf8_text(source) == Ok(expected)
        {
            return true;
        }
        if matches!(child.kind(), "user_type" | "constructor_invocation")
            && has_annotation_name(child, expected, source)
        {
            return true;
        }
    }
    false
}

fn is_factory_method(node: tree_sitter::Node, fn_name: &str, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(user_type) = extract_user_type_node(child) {
            let mut type_cursor = user_type.walk();
            for type_child in user_type.children(&mut type_cursor) {
                if type_child.kind() == "type_identifier"
                    && type_child.utf8_text(source) == Ok(fn_name)
                {
                    return true;
                }
            }
        }
        if child.kind() == "function_body" {
            let mut body_cursor = child.walk();
            for body_child in child.children(&mut body_cursor) {
                if body_child.kind() == "call_expression"
                    && let Some(callee) = body_child.child(0)
                    && callee.utf8_text(source) == Ok(fn_name)
                {
                    return true;
                }
            }
        }
    }
    false
}

fn extract_user_type_node(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if node.kind() == "user_type" {
        return Some(node);
    }
    if node.kind() == "nullable_type" {
        let mut cursor = node.walk();
        return node.children(&mut cursor).find(|c| c.kind() == "user_type");
    }
    None
}

fn is_valid_function_name(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|character| character.is_ascii_alphanumeric())
}

fn is_valid_test_function_name(text: &str) -> bool {
    if text.starts_with('`') && text.ends_with('`') && text.len() >= 2 {
        return true;
    }
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn has_test_library_import(root: tree_sitter::Node, source: &[u8]) -> bool {
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
                            && is_test_package_path(text)
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

fn is_test_package_path(path: &str) -> bool {
    const TEST_PACKAGES: &[&str] = &[
        "org.junit",
        "kotlin.test",
        "io.kotest",
        "junit.framework",
        "org.testng",
    ];
    TEST_PACKAGES
        .iter()
        .any(|&pkg| path == pkg || (path.starts_with(pkg) && path[pkg.len()..].starts_with('.')))
}
