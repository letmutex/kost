use ahash::AHashMap;
use common::concat_str;
use tree_sitter::Node;

pub use super::string::{
    resolve_groovy_string, resolve_interpolated_string, resolve_kotlin_string,
};

/// Extracts string slice from source bytes corresponding to node's byte range.
pub fn node_text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}

/// Finds first direct child node matching the given syntax kind name.
pub fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|&child| child.kind() == kind)
}

/// Check if an identifier name matches a Gradle dependency configuration.
#[inline]
pub fn is_dependency_configuration(name: &str) -> bool {
    matches_config_suffix(name, "implementation", "Implementation")
        || matches_config_suffix(name, "api", "Api")
        || matches_config_suffix(name, "compileOnly", "CompileOnly")
        || matches_config_suffix(name, "runtimeOnly", "RuntimeOnly")
        || matches_config_suffix(name, "annotationProcessor", "AnnotationProcessor")
        || matches_config_suffix(name, "ksp", "Ksp")
        || is_prefixed_config(name, "ksp")
        || matches_config_suffix(name, "kapt", "Kapt")
        || is_prefixed_config(name, "kapt")
        || name == "classpath"
}

#[inline]
fn matches_config_suffix(name: &str, exact_lower: &str, pascal_suffix: &str) -> bool {
    name == exact_lower || name.ends_with(pascal_suffix)
}

#[inline]
fn is_prefixed_config(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.as_bytes().first())
        .is_some_and(u8::is_ascii_uppercase)
}

/// Check if an expression text represents a Gradle version catalog accessor (e.g. `libs.androidx.core.ktx`).
#[inline]
pub fn is_catalog_accessor(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("libs.") || trimmed.starts_with("libraries.")
}

/// Check if a string represents a valid repository URL scheme.
pub fn is_repo_url(val: &str) -> bool {
    val.starts_with("http://") || val.starts_with("https://") || val.starts_with("file:")
}

/// Look up a property by name, stripping common Gradle property receiver prefixes if needed.
pub fn lookup_property<'a>(
    expr: &str,
    properties: &'a AHashMap<String, String>,
) -> Option<&'a String> {
    let trimmed = expr.trim();
    properties.get(trimmed).or_else(|| {
        let stripped = trimmed
            .strip_prefix("rootProject.ext.")
            .or_else(|| trimmed.strip_prefix("rootProject.extra."))
            .or_else(|| trimmed.strip_prefix("project.ext."))
            .or_else(|| trimmed.strip_prefix("project.extra."))
            .or_else(|| trimmed.strip_prefix("ext."))
            .or_else(|| trimmed.strip_prefix("extra."))
            .or_else(|| trimmed.strip_prefix("project."))?;
        properties.get(stripped)
    })
}

/// Unpacks a Kotlin `value_argument` into `(optional_param_name, value_expr_node)`.
pub fn unpack_kotlin_value_argument<'a>(
    arg_node: Node<'a>,
    source: &'a [u8],
) -> Option<(Option<&'a str>, Node<'a>)> {
    let mut cursor = arg_node.walk();
    let mut has_equals = false;
    let mut first_ident: Option<&'a str> = None;
    let mut last_named: Option<Node<'a>> = None;

    for child in arg_node.children(&mut cursor) {
        if child.kind() == "=" {
            has_equals = true;
        } else if child.is_named() {
            if !has_equals && first_ident.is_none() && child.kind() == "simple_identifier" {
                first_ident = Some(node_text(child, source));
            }
            last_named = Some(child);
        }
    }

    let value_expr = last_named?;
    let param_name = if has_equals { first_ident } else { None };
    Some((param_name, value_expr))
}

/// Resolves a Kotlin expression (string literal, property identifier, indexing, or `+` concatenation).
pub fn resolve_kotlin_expr(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
) -> Option<String> {
    match node.kind() {
        "string_literal" => Some(resolve_kotlin_string(node, source, properties)),
        "simple_identifier" | "navigation_expression" => {
            lookup_property(node_text(node, source), properties).cloned()
        }
        "indexing_expression" => resolve_kotlin_indexing_expr(node, source, properties),
        "additive_expression" => resolve_binary_concat(node, source, properties, true),
        _ => None,
    }
}

fn resolve_kotlin_indexing_expr(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
) -> Option<String> {
    let target = node.child(0)?;
    let target_text = node_text(target, source).trim();
    if target_text != "extra"
        && target_text != "ext"
        && !target_text.ends_with(".extra")
        && !target_text.ends_with(".ext")
    {
        return None;
    }
    let suffix = find_child_by_kind(node, "indexing_suffix")?;
    let key_node = find_child_by_kind(suffix, "string_literal")?;
    let key = resolve_kotlin_string(key_node, source, properties);
    properties.get(&key).cloned()
}

/// Resolves a Groovy expression (string literal, property identifier, or `+` concatenation).
pub fn resolve_groovy_expr(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
) -> Option<String> {
    match node.kind() {
        "string_literal" => Some(resolve_groovy_string(node, source, properties)),
        "identifier" | "field_access" => {
            lookup_property(node_text(node, source), properties).cloned()
        }
        "binary_expression" => resolve_binary_concat(node, source, properties, false),
        _ => None,
    }
}

fn resolve_binary_concat(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    is_kotlin: bool,
) -> Option<String> {
    let left = node.child(0)?;
    let op = node.child(1)?;
    let right = node.child(2)?;
    if node_text(op, source).trim() != "+" {
        return None;
    }
    let left_val = if is_kotlin {
        resolve_kotlin_expr(left, source, properties)?
    } else {
        resolve_groovy_expr(left, source, properties)?
    };
    let right_val = if is_kotlin {
        resolve_kotlin_expr(right, source, properties)?
    } else {
        resolve_groovy_expr(right, source, properties)?
    };
    Some(concat_str!(&left_val, &right_val))
}

/// Check if a Groovy AST contains unrecoverable syntax errors (ignoring bare map comma separators).
pub fn has_unrecoverable_groovy_error(node: Node, source: &[u8]) -> bool {
    if !node.has_error() {
        return false;
    }
    if node.is_missing() {
        return true;
    }
    if node.kind() == "ERROR" {
        return node_text(node, source).trim() != ",";
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_unrecoverable_groovy_error(child, source) {
            return true;
        }
    }
    false
}

/// Extracts key-value coordinates from a Groovy named argument or map entry.
pub fn extract_named_pair(
    pair_node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
    group: &mut Option<String>,
    name: &mut Option<String>,
    version: &mut Option<String>,
) {
    let mut cursor = pair_node.walk();
    let mut key_node: Option<Node> = None;
    let mut val_node: Option<Node> = None;

    for child in pair_node.children(&mut cursor) {
        if child.is_named() {
            if key_node.is_none() {
                key_node = Some(child);
            } else {
                val_node = Some(child);
            }
        }
    }

    let (Some(key_child), Some(val_child)) = (key_node, val_node) else {
        return;
    };

    let key = node_text(key_child, source).trim_matches(['\'', '"']);
    let Some(val) = resolve_groovy_expr(val_child, source, properties) else {
        return;
    };

    match key {
        "group" => *group = Some(val),
        "name" => *name = Some(val),
        "version" => *version = Some(val),
        _ => {}
    }
}
