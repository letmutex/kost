use ahash::AHashMap;
use common::concat_str;
use tree_sitter::Node;

use super::util::{lookup_property, node_text, resolve_kotlin_expr};

/// Interpolate `${var}` and `$var` references inside a raw string using `vars`.
pub fn resolve_interpolated_string(input: &str, vars: &AHashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, val) in vars {
        let braced = concat_str!("${", key, "}");
        if result.contains(&braced) {
            result = result.replace(&braced, val);
        }
        let unbraced = concat_str!("$", key);
        if result.contains(&unbraced) {
            result = result.replace(&unbraced, val);
        }
    }
    result
}

/// Resolves a Kotlin string literal node, interpolating variables and unescaping characters.
pub fn resolve_kotlin_string(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
) -> String {
    let raw_node = node_text(node, source);
    let is_raw_string = raw_node.starts_with("\"\"\"");
    let mut result = String::new();
    let mut cursor = node.walk();
    let mut has_content = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_content" => {
                has_content = true;
                let text = node_text(child, source);
                if is_raw_string || !text.contains('\\') {
                    result.push_str(text);
                } else {
                    append_unescaped_text(&mut result, text);
                }
            }
            "character_escape_seq" => {
                has_content = true;
                append_unescaped_text(&mut result, node_text(child, source));
            }
            "interpolated_identifier" => {
                has_content = true;
                let ident = node_text(child, source);
                if let Some(val) = lookup_property(ident, properties) {
                    result.push_str(val);
                } else {
                    result.push('$');
                    result.push_str(ident);
                }
            }
            "interpolated_expression" => {
                has_content = true;
                let resolved = child
                    .named_child(0)
                    .and_then(|inner| resolve_kotlin_expr(inner, source, properties))
                    .or_else(|| {
                        let expr = node_text(child, source).trim();
                        lookup_property(expr, properties).cloned()
                    });
                if let Some(val) = resolved {
                    result.push_str(&val);
                } else {
                    let expr = node_text(child, source).trim();
                    result.push_str("${");
                    result.push_str(expr);
                    result.push('}');
                }
            }
            _ => {}
        }
    }

    if !has_content {
        let trimmed = raw_node
            .strip_prefix("\"\"\"")
            .and_then(|inner| inner.strip_suffix("\"\"\""))
            .or_else(|| {
                raw_node
                    .strip_prefix('"')
                    .and_then(|inner| inner.strip_suffix('"'))
            })
            .unwrap_or(raw_node);
        if !is_raw_string && trimmed.contains('\\') {
            append_unescaped_text(&mut result, trimmed);
            return result;
        }
        return trimmed.to_string();
    }

    result
}

/// Resolves a Groovy string literal or GString node, interpolating variables if present.
pub fn resolve_groovy_string(
    node: Node,
    source: &[u8],
    properties: &AHashMap<String, String>,
) -> String {
    let mut result = String::new();
    let mut cursor = node.walk();
    let mut has_content = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_fragment" => {
                has_content = true;
                let text = node_text(child, source);
                if text.contains('\\') {
                    append_unescaped_text(&mut result, text);
                } else {
                    result.push_str(text);
                }
            }
            "escape_sequence" => {
                has_content = true;
                append_unescaped_text(&mut result, node_text(child, source));
            }
            "gstring_dollar_interpolation" | "gstring_brace_interpolation" => {
                has_content = true;
                let raw_interp = node_text(child, source);
                let ident = raw_interp
                    .strip_prefix("${")
                    .and_then(|inner| inner.strip_suffix('}'))
                    .or_else(|| raw_interp.strip_prefix('$'))
                    .unwrap_or(raw_interp)
                    .trim();
                if let Some(val) = lookup_property(ident, properties) {
                    result.push_str(val);
                } else {
                    result.push_str(raw_interp);
                }
            }
            _ => {}
        }
    }

    if !has_content {
        let raw = node_text(node, source);
        let trimmed = strip_groovy_quotes(raw);
        if trimmed.contains('\\') {
            append_unescaped_text(&mut result, trimmed);
            return result;
        }
        return trimmed.to_string();
    }

    result
}

fn strip_groovy_quotes(raw: &str) -> &str {
    for delim in ["'''", "\"\"\"", "'", "\""] {
        if let Some(inner) = raw
            .strip_prefix(delim)
            .and_then(|rest| rest.strip_suffix(delim))
        {
            return inner;
        }
    }
    raw
}

fn append_unescaped_text(result: &mut String, text: &str) {
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some(other) => result.push(other),
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
}
