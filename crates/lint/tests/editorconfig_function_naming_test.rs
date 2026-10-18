use common::editorconfig::ResolvedEditorConfig;
use lint::Linter;

#[test]
fn test_lint_respects_editorconfig_function_naming_ignore_when_annotated_with() {
    let linter = Linter::standard();
    let code = "@Test\nfun Result_catches_errors() {}\n";

    let ini = "[*.kt]\nktlint_function_naming_ignore_when_annotated_with = Composable, Test\n";
    let file = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    common::editorconfig::apply_file_properties(&file, "src/ResultTest.kt", &mut config);

    let report = linter.lint_str_with_config(code, Some("src/ResultTest.kt"), Some(&config));
    assert!(
        report.is_clean(),
        "Expected clean report when annotated with @Test, but got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn test_lint_respects_editorconfig_standard_namespaced_function_naming_ignore() {
    let linter = Linter::standard();
    let code = "@Test\nfun Result_catches_errors() {}\n";

    let ini =
        "[*.kt]\nktlint_standard_function_naming_ignore_when_annotated_with = Composable, Test\n";
    let file = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    common::editorconfig::apply_file_properties(&file, "src/ResultTest.kt", &mut config);

    let report = linter.lint_str_with_config(code, Some("src/ResultTest.kt"), Some(&config));
    assert!(
        report.is_clean(),
        "Expected clean report when annotated with @Test, but got diagnostics: {:?}",
        report.diagnostics()
    );
}
