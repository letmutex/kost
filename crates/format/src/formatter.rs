//! Strongly-typed, filesystem-free Kotlin code formatting engine facade.

use crate::diff::create_diff;
use crate::error::FormatError;
use crate::options::{FileType, FormattingOptions, StylePreset};

/// Structured outcome of a format verification check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    /// True if the source is already 100% formatted.
    pub is_formatted: bool,
    /// Unified diff if unformatted, or `None` if clean.
    pub diff: Option<String>,
}

/// An immutable, thread-safe Kotlin code formatter.
///
/// Designed to be shared across threads or cloned cheaply.
#[derive(Debug, Clone)]
pub struct Formatter {
    options: FormattingOptions,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::from_preset(StylePreset::Kotlinlang)
    }
}

impl Formatter {
    /// Constructs a formatter with the default Kotlinlang style preset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs a formatter with explicit custom options.
    pub fn with_options(options: FormattingOptions) -> Self {
        Self { options }
    }

    /// Constructs a formatter from a specified style preset.
    pub fn from_preset(preset: StylePreset) -> Self {
        Self {
            options: FormattingOptions::from_preset(preset),
        }
    }

    /// Returns a reference to the active formatting options.
    pub fn options(&self) -> &FormattingOptions {
        &self.options
    }

    /// Formats Kotlin regular source code in memory.
    pub fn format(&self, source: &str) -> Result<String, FormatError> {
        crate::format(source, &self.options)
    }

    /// Formats Kotlin source code with explicit `FileType` (Regular vs Script).
    pub fn format_file(&self, source: &str, file_type: FileType) -> Result<String, FormatError> {
        crate::format_file(source, file_type, &self.options)
    }

    /// Verifies if source code is formatted without unnecessary allocations if already clean.
    pub fn check(
        &self,
        source: &str,
        file_type: FileType,
        file_label: &str,
        colorize: bool,
    ) -> Result<CheckResult, FormatError> {
        let formatted = self.format_file(source, file_type)?;
        if formatted == source {
            Ok(CheckResult {
                is_formatted: true,
                diff: None,
            })
        } else {
            let diff = create_diff(source, &formatted, file_label, colorize);
            Ok(CheckResult {
                is_formatted: false,
                diff,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Formatter;
    use crate::options::FileType;

    #[test]
    fn test_formatter_default_uses_kotlinlang_indent() {
        let formatter = Formatter::new();
        assert_eq!(formatter.options().block_indent, 4);
        assert_eq!(formatter.options().continuation_indent, 4);
    }

    #[test]
    fn test_formatter_format_clean_code() {
        let formatter = Formatter::new();
        let source = "fun main() {\n    println(\"hello\")\n}\n";
        let formatted = formatter.format(source).unwrap();
        assert_eq!(formatted, source);
    }

    #[test]
    fn test_formatter_check_detects_unformatted_code_with_diff() {
        let formatter = Formatter::new();
        let unformatted = "fun main() {\n  println(\"hello\")\n}\n";
        let result = formatter
            .check(unformatted, FileType::Regular, "Main.kt", false)
            .unwrap();

        assert!(!result.is_formatted);
        assert!(result.diff.is_some());
        let diff_text = result.diff.unwrap();
        assert!(diff_text.contains("-  println(\"hello\")\n"));
        assert!(diff_text.contains("+    println(\"hello\")\n"));
    }

    #[test]
    fn test_formatter_check_clean_code_has_no_diff() {
        let formatter = Formatter::new();
        let clean = "fun main() {\n    println(\"hello\")\n}\n";
        let result = formatter
            .check(clean, FileType::Regular, "Main.kt", false)
            .unwrap();

        assert!(result.is_formatted);
        assert_eq!(result.diff, None);
    }

    #[test]
    fn test_empty_companion_object_parse() {
        let mut kp = crate::engine::KotlinParser::new();
        let source = "class Foo {\n  val a: String\n\n  companion object;\n\n  init {\n    a = \"Hello\"\n  }\n}\n";
        let tree = kp.parse(source).unwrap();
        fn print_node(node: tree_sitter::Node<'_>, src: &str, depth: usize) {
            println!(
                "{}{:?}: {:?}",
                "  ".repeat(depth),
                node.kind(),
                &src[node.start_byte()..node.end_byte()]
            );
            let mut cursor = node.walk();
            for c in node.children(&mut cursor) {
                print_node(c, src, depth + 1);
            }
        }
        print_node(tree.root_node(), source, 0);

        let _formatter = Formatter::from_preset(crate::options::StylePreset::Meta);
        let redundant =
            crate::rules::semicolons::find_redundant_semicolons(tree.root_node(), source);
        println!("REDUNDANT OFFSETS: {:?}", redundant);
    }

    #[test]
    fn test_anonymous_function_in_argument_formatting() {
        let source = "// TRAILING_COMMA_STRATEGY NONE\n\nfun f() {\n  setListener(\n      fun(number: Int) {\n        println(number)\n      })\n}\n";
        let mut opts = crate::options::FormattingOptions::meta();
        opts.trailing_comma_strategy = crate::options::TrailingCommaStrategy::None;
        let formatted =
            crate::engine::format_source(source, crate::options::FileType::Regular, &opts).unwrap();
        assert_eq!(formatted, source);
    }
}
