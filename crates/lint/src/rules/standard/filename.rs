//! Built-in AST rule enforcing Kotlin filename conventions (Ktlint rule 1).

use std::path::Path;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces matching filenames for single top-level declarations or PascalCase.
pub struct Filename;

impl AstRule for Filename {
    fn id(&self) -> &'static str {
        "filename"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Some(file_path_str) = ctx.file_path.as_deref() else {
            return;
        };

        let file_path = Path::new(file_path_str);
        let Some(extension) = file_path.extension().and_then(|s| s.to_str()) else {
            return;
        };

        if extension != "kt" {
            return;
        }

        let Some(file_name) = file_path.file_name().and_then(|s| s.to_str()) else {
            return;
        };

        let Some(file_stem) = file_path.file_stem().and_then(|s| s.to_str()) else {
            return;
        };

        if has_jvm_file_name_override(node, source) {
            return;
        }

        let normalized_stem = strip_kmp_target_suffix(file_stem);
        let top_classes = collect_top_level_class_names(node, source);
        let has_other_top_level_declarations = has_other_top_level_decls(node);

        if top_classes.len() == 1 && !has_other_top_level_declarations {
            let (class_name, class_node) = &top_classes[0];
            if !is_matching_filename(normalized_stem, class_name) {
                let message = common::concat_str!(
                    "File '",
                    file_name,
                    "' contains a single class and should be named '",
                    class_name,
                    ".kt'"
                );
                ctx.report(*class_node, message);
            }
        } else if !is_pascal_case(normalized_stem) {
            let message =
                common::concat_str!("File '", file_name, "' should be named in PascalCase");
            ctx.report(node, message);
        }
    }
}

fn has_other_top_level_decls(root: tree_sitter::Node) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(
            child.kind(),
            "function_declaration" | "property_declaration" | "typealias_declaration"
        ) {
            return true;
        }
    }
    false
}

fn has_jvm_file_name_override(root: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "file_annotation" && has_matching_annotation_identifier(child, source) {
            return true;
        }
    }
    false
}

fn has_matching_annotation_identifier(node: tree_sitter::Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "type_identifier" | "simple_identifier")
            && let Ok(name) = child.utf8_text(source)
            && matches!(name, "JvmName" | "JvmMultifileClass")
        {
            return true;
        }
        if has_matching_annotation_identifier(child, source) {
            return true;
        }
    }
    false
}

fn collect_top_level_class_names<'a>(
    root: tree_sitter::Node<'a>,
    source: &'a [u8],
) -> Vec<(&'a str, tree_sitter::Node<'a>)> {
    let mut classes = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if matches!(child.kind(), "class_declaration" | "object_declaration") {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if matches!(inner.kind(), "type_identifier" | "simple_identifier")
                    && let Ok(name) = inner.utf8_text(source)
                {
                    classes.push((name, child));
                    break;
                }
            }
        }
    }
    classes
}

fn is_matching_filename(normalized_stem: &str, class_name: &str) -> bool {
    if let Some(suffix) = normalized_stem.strip_prefix(class_name) {
        matches!(suffix, "" | "Test" | "Tests" | "Spec")
    } else {
        false
    }
}

fn strip_kmp_target_suffix(file_stem: &str) -> &str {
    const KMP_TARGETS: &[&str] = &[
        "jvm", "android", "ios", "native", "js", "wasm", "apple", "linux", "mingw", "desktop",
        "common",
    ];

    for target in KMP_TARGETS {
        if let Some(prefix) = file_stem.strip_suffix(target)
            && let Some(stripped) = prefix
                .strip_suffix('.')
                .or_else(|| prefix.strip_suffix('_'))
        {
            return stripped;
        }
    }
    file_stem
}

fn is_pascal_case(string: &str) -> bool {
    let mut chars = string.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}
