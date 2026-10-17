use common::editorconfig::{LineLengthSetting, ResolvedEditorConfig};
use lint::Linter;
use smol_str::SmolStr;

#[test]
fn test_lint_respects_editorconfig_max_line_length_override() {
    let linter = Linter::standard();
    // 70 characters line
    let code = "val myLongVariableIdentifierName = \"Some text here that makes it long\"\n";

    // Standard default limit is 140 -> clean
    let clean_report = linter.lint_str(code);
    assert!(clean_report.is_clean());

    // With EditorConfig limit = 60 -> violation
    let mut config = ResolvedEditorConfig::new();
    config.max_line_length = Some(LineLengthSetting::Value(60));

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert_eq!(report.error_count() + report.warning_count(), 1);
    assert_eq!(
        report.diagnostics()[0].message,
        "Exceeded max line length (60)"
    );
}

#[test]
fn test_lint_respects_editorconfig_max_line_length_disabled() {
    let linter = Linter::standard();
    let long_line = "val x = \"".to_string() + &"a".repeat(160) + "\"\n";

    // Standard default triggers max-line-length (140)
    let default_report = linter.lint_str(&long_line);
    assert!(
        default_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:max-line-length")
    );

    // With EditorConfig max_line_length = off -> no max-line-length violation
    let mut config = ResolvedEditorConfig::new();
    config.max_line_length = Some(LineLengthSetting::Off);

    let report = linter.lint_str_with_config(&long_line, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:max-line-length")
    );
}

#[test]
fn test_lint_disables_ast_rule_via_editorconfig_toggle() {
    let linter = Linter::standard();
    let code = "fun main() {\n    val a = 1; ; val b = 2\n}\n";

    // Default reports unnecessary semicolon
    let default_report = linter.lint_str(code);
    assert!(
        default_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:no-semi")
    );

    // Disable no-semi via EditorConfig
    let mut config = ResolvedEditorConfig::new();
    config
        .rule_toggles
        .insert(SmolStr::new("ktlint_standard_no-semi"), false);

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:no-semi")
    );
}

#[test]
fn test_lint_disables_entire_standard_ruleset_via_editorconfig() {
    let linter = Linter::standard();
    let code = "fun main() {\n    val a = 1; ; val b = 2\n}";

    let mut config = ResolvedEditorConfig::new();
    config.ktlint_standard_enabled = Some(false);

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert!(report.is_clean());
}

#[test]
fn test_lint_re_enables_specific_rule_when_ruleset_disabled() {
    let linter = Linter::standard();
    let code = "fun main() {\n    val a = 1; ; val b = 2\n}\n";

    let mut config = ResolvedEditorConfig::new();
    config.ktlint_standard_enabled = Some(false);
    config
        .rule_toggles
        .insert(SmolStr::new("ktlint_standard_no-semi"), true);

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert_eq!(report.diagnostics().len(), 1);
    assert_eq!(report.diagnostics()[0].rule_id, "standard:no-semi");
}

#[test]
fn test_lint_respects_editorconfig_final_newline_disabled() {
    let linter = Linter::standard();
    let code = "val x = 1";

    let default_report = linter.lint_str(code);
    assert!(
        default_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:final-newline")
    );

    let mut config = ResolvedEditorConfig::new();
    config.insert_final_newline = Some(false);

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:final-newline")
    );
}

#[test]
fn test_lint_respects_editorconfig_trailing_comma_call_site_flag() {
    let linter = Linter::standard();
    let multiline_call_without_comma = "fun test() {\n    foo(\n        1,\n        2\n    )\n}\n";

    // Default expects trailing comma on multiline call site
    let default_report = linter.lint_str(multiline_call_without_comma);
    assert!(
        default_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:trailing-comma-on-call-site")
    );

    // When allow_trailing_comma_on_call_site = false -> missing comma is NOT reported
    let mut config = ResolvedEditorConfig::new();
    config.ij_allow_trailing_comma_on_call_site = Some(false);

    let report = linter.lint_str_with_config(
        multiline_call_without_comma,
        Some("src/Main.kt"),
        Some(&config),
    );
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:trailing-comma-on-call-site")
    );
}

#[test]
fn test_lint_call_site_inherits_declaration_site_trailing_comma_flag() {
    let linter = Linter::standard();
    let multiline_call = "fun test() {\n    foo(\n        1,\n        2\n    )\n}\n";

    // Set only ij_allow_trailing_comma = false; call-site should inherit it
    let mut config = ResolvedEditorConfig::new();
    config.ij_allow_trailing_comma = Some(false);

    let report = linter.lint_str_with_config(multiline_call, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:trailing-comma-on-call-site")
    );
}

#[test]
fn test_lint_disables_ast_rule_via_underscore_rule_toggle() {
    let linter = Linter::standard();
    let code = "fun main() {\n    val a = 1; ; val b = 2\n}\n";

    // Disable standard:no-semi using underscore format (ktlint_standard_no_semi)
    let ini = "[*.kt]\nktlint_standard_no_semi = disabled\n";
    let parsed = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    for section in &parsed.sections {
        for (k, v) in &section.properties {
            if let Some(suffix) = k.strip_prefix("ktlint_standard_") {
                let key = common::concat_smol_str!("ktlint_standard_", &suffix.replace('_', "-"));
                config.rule_toggles.insert(key, v.as_str() == "enabled");
            }
        }
    }

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:no-semi")
    );
}

#[test]
fn test_lint_respects_editorconfig_trailing_comma_declaration_site_disabled() {
    let linter = Linter::standard();
    let multiline_param_with_comma = "fun foo(\n    a: Int,\n    b: Int,\n) {}\n";
    let multiline_param_without_comma = "fun foo(\n    a: Int,\n    b: Int\n) {}\n";

    let mut config = ResolvedEditorConfig::new();
    config.ij_allow_trailing_comma = Some(false);

    let report_with_comma = linter.lint_str_with_config(
        multiline_param_with_comma,
        Some("src/Main.kt"),
        Some(&config),
    );
    assert!(report_with_comma.diagnostics().iter().any(|d| d.rule_id
        == "standard:trailing-comma-on-declaration-site"
        && d.message.contains("Unnecessary trailing comma")));

    let report_without_comma = linter.lint_str_with_config(
        multiline_param_without_comma,
        Some("src/Main.kt"),
        Some(&config),
    );
    assert!(
        !report_without_comma
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:trailing-comma-on-declaration-site")
    );
}

#[test]
fn test_lint_respects_editorconfig_trim_trailing_whitespace_disabled() {
    let linter = Linter::standard();
    let code_with_trailing = "fun main() {   \n}\n";

    let default_report = linter.lint_str(code_with_trailing);
    assert!(
        default_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:no-trailing-spaces")
    );

    let mut config = ResolvedEditorConfig::new();
    config.trim_trailing_whitespace = Some(false);

    let report =
        linter.lint_str_with_config(code_with_trailing, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:no-trailing-spaces")
    );
}

#[test]
fn test_lint_respects_editorconfig_indent_style_tab() {
    let linter = Linter::standard();
    let tab_indented_code = "fun main() {\n\tprintln(\"hello\")\n}\n";

    let default_report = linter.lint_str(tab_indented_code);
    assert!(
        default_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:indent" && d.message.contains("Unexpected tab"))
    );

    let mut config = ResolvedEditorConfig::new();
    config.indent_style = Some(common::editorconfig::IndentStyle::Tab);

    let report = linter.lint_str_with_config(tab_indented_code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:indent" && d.message.contains("Unexpected tab"))
    );
}
