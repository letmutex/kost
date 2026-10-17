use common::editorconfig::{EditorConfigFile, EndOfLine, LineLengthSetting, ResolvedEditorConfig};
use format::options::{FormattingOptions, StylePreset};
use format::{Formatter, format_file};

#[test]
fn test_format_respects_editorconfig_indent_size() {
    let mut config = ResolvedEditorConfig::new();
    config.indent_size = Some(2);
    config.ij_continuation_indent_size = Some(2);

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    assert_eq!(options.block_indent, 2);
    assert_eq!(options.continuation_indent, 2);

    let formatter = Formatter::with_options(options);
    let code = "fun main() {\nprintln(\"hello\")\n}\n";
    let formatted = formatter.format(code).unwrap();
    assert_eq!(formatted, "fun main() {\n  println(\"hello\")\n}\n");
}

#[test]
fn test_format_respects_editorconfig_max_line_length() {
    let mut config = ResolvedEditorConfig::new();
    config.max_line_length = Some(LineLengthSetting::Value(50));

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    assert_eq!(options.max_width, 50);
}

#[test]
fn test_format_respects_editorconfig_insert_final_newline_disabled() {
    let mut config = ResolvedEditorConfig::new();
    config.insert_final_newline = Some(false);

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    let code = "fun main() {\n    println(\"hello\")\n}\n";
    let formatted = format_file(code, format::options::FileType::Regular, &options).unwrap();
    assert!(!formatted.ends_with('\n'));
    assert_eq!(formatted, "fun main() {\n    println(\"hello\")\n}");
}

#[test]
fn test_format_respects_editorconfig_end_of_line_crlf() {
    let mut config = ResolvedEditorConfig::new();
    config.end_of_line = Some(EndOfLine::Crlf);

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    let code = "fun main() {\n    println(\"hello\")\n}\n";
    let formatted = format_file(code, format::options::FileType::Regular, &options).unwrap();
    assert!(formatted.contains("\r\n"));
}

#[test]
fn test_format_respects_editorconfig_section_matching() {
    let ini = r#"
root = true
[*.kt]
indent_size = 2
insert_final_newline = false
"#;
    let file = EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    for section in &file.sections {
        if section.matcher.is_match("src/App.kt") {
            for (k, v) in &section.properties {
                if k.as_str() == "indent_size" {
                    config.indent_size = v.parse().ok();
                } else if k.as_str() == "insert_final_newline" {
                    config.insert_final_newline = Some(v.as_str() == "true");
                }
            }
        }
    }

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    let code = "fun test() {\nval x = 1\n}\n";
    let formatted = format_file(code, format::options::FileType::Regular, &options).unwrap();
    assert_eq!(formatted, "fun test() {\n  val x = 1\n}");
}

#[test]
fn test_format_respects_editorconfig_end_of_line_cr() {
    let mut config = ResolvedEditorConfig::new();
    config.end_of_line = Some(EndOfLine::Cr);

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    let code = "fun main() {\n    println(\"hello\")\n}\n";
    let formatted = format_file(code, format::options::FileType::Regular, &options).unwrap();
    assert!(formatted.contains('\r'));
    assert!(!formatted.contains("\r\n"));
    assert!(!formatted.contains('\n'));
}

#[test]
fn test_format_trailing_comma_strategy_inherits_declaration_site_flag() {
    let mut config = ResolvedEditorConfig::new();
    config.ij_allow_trailing_comma = Some(false);

    let options = FormattingOptions::from_editorconfig(StylePreset::Kotlinlang, &config);
    assert_eq!(
        options.trailing_comma_strategy,
        format::options::TrailingCommaStrategy::None
    );
}
