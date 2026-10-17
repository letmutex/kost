//! Parsing of .editorconfig file content into structured sections and properties.

use crate::editorconfig::glob::compile_glob;
use regex::Regex;
use smol_str::SmolStr;

/// Parsed representation of a single section within an `.editorconfig` file.
#[derive(Debug, Clone)]
pub struct EditorConfigSection {
    pub pattern: SmolStr,
    pub matcher: Regex,
    pub properties: Vec<(SmolStr, SmolStr)>,
}

/// Structured representation of an entire parsed `.editorconfig` file.
#[derive(Debug, Clone)]
pub struct EditorConfigFile {
    pub is_root: bool,
    pub sections: Vec<EditorConfigSection>,
}

impl EditorConfigFile {
    /// Parses the raw string contents of an `.editorconfig` file.
    pub fn parse(content: &str) -> Result<Self, String> {
        let clean_content = content.strip_prefix('\u{feff}').unwrap_or(content);
        let mut is_root = false;
        let mut sections = Vec::new();
        let mut current_section: Option<EditorConfigSection> = None;

        for line in clean_content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
                if let Some(section) = current_section.take() {
                    sections.push(section);
                }
                let pattern_str = trimmed[1..trimmed.len() - 1].trim();
                let matcher = compile_glob(pattern_str)?;
                current_section = Some(EditorConfigSection {
                    pattern: SmolStr::new(pattern_str),
                    matcher,
                    properties: Vec::new(),
                });
            } else if let Some((key, val)) = parse_key_value(trimmed) {
                if let Some(section) = current_section.as_mut() {
                    section.properties.push((key, val));
                } else if key == "root" && val.eq_ignore_ascii_case("true") {
                    is_root = true;
                }
            }
        }

        if let Some(section) = current_section {
            sections.push(section);
        }

        Ok(Self { is_root, sections })
    }
}

fn parse_key_value(line: &str) -> Option<(SmolStr, SmolStr)> {
    let separator_pos = match (line.find('='), line.find(':')) {
        (Some(eq), Some(colon)) => Some(eq.min(colon)),
        (a, b) => a.or(b),
    }?;
    let key = line[..separator_pos].trim().to_ascii_lowercase();
    let raw_val = line[separator_pos + 1..].trim();

    if key.is_empty() {
        return None;
    }

    let value = strip_inline_comment(raw_val);
    Some((SmolStr::new(&key), SmolStr::new(value)))
}

fn strip_inline_comment(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut prev_is_whitespace = false;
    let mut is_escaped = false;

    for (i, &b) in bytes.iter().enumerate() {
        if is_escaped {
            is_escaped = false;
            prev_is_whitespace = b.is_ascii_whitespace();
            continue;
        }
        if b == b'\\' {
            is_escaped = true;
            prev_is_whitespace = false;
            continue;
        }
        if (b == b'#' || b == b';') && (i == 0 || prev_is_whitespace) {
            return value[..i].trim_end();
        }
        prev_is_whitespace = b.is_ascii_whitespace();
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{EditorConfigFile, parse_key_value};
    use smol_str::SmolStr;

    #[test]
    fn test_editorconfig_parse_sections_and_properties() {
        let content = r#"
# Top level comment
root = true

[*.{kt,kts}]
indent_style = space
indent_size = 4
max_line_length = 120
insert_final_newline = true

[src/test/**.kt]
max_line_length = 160
ktlint_standard_no-semi = disabled
"#;

        let parsed = EditorConfigFile::parse(content).unwrap();
        assert!(parsed.is_root);
        assert_eq!(parsed.sections.len(), 2);

        let first = &parsed.sections[0];
        assert_eq!(first.pattern.as_str(), "*.{kt,kts}");
        assert_eq!(first.properties.len(), 4);
        assert_eq!(first.properties[0].0.as_str(), "indent_style");
        assert_eq!(first.properties[0].1.as_str(), "space");

        let second = &parsed.sections[1];
        assert_eq!(second.pattern.as_str(), "src/test/**.kt");
        assert_eq!(second.properties.len(), 2);
        assert_eq!(
            second.properties[1],
            (
                SmolStr::new("ktlint_standard_no-semi"),
                SmolStr::new("disabled")
            )
        );
    }

    #[test]
    fn test_parse_key_value_inline_comments_and_separators() {
        let kv1 = parse_key_value("indent_size = 4 # standard indent").unwrap();
        assert_eq!(kv1.0.as_str(), "indent_size");
        assert_eq!(kv1.1.as_str(), "4");

        let kv2 = parse_key_value("max_line_length = 120 ; inline semicolon").unwrap();
        assert_eq!(kv2.0.as_str(), "max_line_length");
        assert_eq!(kv2.1.as_str(), "120");

        let kv3 = parse_key_value("key: value=1").unwrap();
        assert_eq!(kv3.0.as_str(), "key");
        assert_eq!(kv3.1.as_str(), "value=1");

        let kv4 = parse_key_value("rules = standard:indent, standard:semi # disable").unwrap();
        assert_eq!(kv4.0.as_str(), "rules");
        assert_eq!(kv4.1.as_str(), "standard:indent, standard:semi");

        let kv5 = parse_key_value(r"escaped = value\#not_a_comment").unwrap();
        assert_eq!(kv5.0.as_str(), "escaped");
        assert_eq!(kv5.1.as_str(), r"value\#not_a_comment");
    }
}
