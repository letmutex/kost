use lint::LintReport;
use lint::smart_lint::{DisplayTier, render_smart_lint_reports};
use lint::types::{Diagnostic, Severity, TextRange};
use smol_str::SmolStr;

fn make_diagnostic(rule_id: &str, severity: Severity, line: u32, message: &str) -> Diagnostic {
    let range = TextRange::new(line, 1, line, 10);
    Diagnostic {
        rule_id: SmolStr::new(rule_id),
        severity,
        check_range: range,
        report_ranges: smallvec::smallvec![range],
        message: message.to_string(),
    }
}

fn set_diagnostic_range(diag: &mut Diagnostic, range: TextRange) {
    diag.check_range = range;
    diag.report_ranges = smallvec::smallvec![range];
}

#[test]
fn test_full_mode_snippet_caret_alignment() {
    let source = "val xVar = 10\n val yVar = \"10\"\n";
    let mut diag = make_diagnostic(
        "standard:property-naming",
        Severity::Error,
        2,
        "Property name should be in camelCase",
    );
    set_diagnostic_range(&mut diag, TextRange::new(2, 2, 2, 10));

    let report = LintReport::new(vec![diag], Some("src/Test.kt".to_string())).with_source(source);

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("1 | val xVar = 10\n"));
    assert!(text.contains("2 |  val yVar = \"10\"\n"));
    assert!(text.contains("     ^\n"));
    assert!(
        text.contains("error: standard:property-naming: Property name should be in camelCase\n")
    );
    assert!(text.contains("    at src/Test.kt:2:2\n"));
}

#[test]
fn test_full_mode_snippet_trim_leading_consecutive_empty_lines() {
    let source = "package com.example\n\n\n\nval y = 10\nval x = y\n";
    let mut diag = make_diagnostic(
        "standard:property-naming",
        Severity::Error,
        6,
        "Property name violation",
    );
    set_diagnostic_range(&mut diag, TextRange::new(6, 1, 6, 2));

    let report = LintReport::new(vec![diag], Some("src/Test.kt".to_string())).with_source(source);

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(!text.contains("4 |"));
    assert!(text.contains("5 | val y = 10\n"));
    assert!(text.contains("6 | val x = y\n"));
    assert!(text.contains("    ^\n"));
    assert!(text.contains("error: standard:property-naming: Property name violation\n"));
    assert!(text.contains("    at src/Test.kt:6:1\n"));
}

#[test]
fn test_full_mode_snippet_preserve_interleaved_empty_lines() {
    let source = "package com.example\nimport a.b.c\nclass Service {\n    val y = 10\n    val z = 20\n    val x = y\n\n    fun run() {}\n}\n";
    let mut diag = make_diagnostic(
        "standard:function-naming",
        Severity::Error,
        8,
        "Function name check",
    );
    set_diagnostic_range(&mut diag, TextRange::new(8, 5, 8, 8));

    let report =
        LintReport::new(vec![diag], Some("src/Service.kt".to_string())).with_source(source);

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("6 |     val x = y\n"));
    assert!(text.contains("7 |\n"));
    assert!(text.contains("8 |     fun run() {}\n"));
    assert!(text.contains("        ^\n"));
    assert!(text.contains("error: standard:function-naming: Function name check\n"));
    assert!(text.contains("    at src/Service.kt:8:5\n"));
}

#[test]
fn test_full_mode_snippet_multiline_blank_line_separation() {
    let source = "val x = 1\nval y = 2\n\n\n\nval name = \"test\"\n";
    let mut diag1 = make_diagnostic("standard:property-naming", Severity::Warning, 1, "Naming 1");
    set_diagnostic_range(&mut diag1, TextRange::new(1, 1, 1, 4));
    let mut diag2 = make_diagnostic("standard:no-semi", Severity::Error, 6, "Semi 2");
    set_diagnostic_range(&mut diag2, TextRange::new(6, 1, 6, 5));

    let report =
        LintReport::new(vec![diag1, diag2], Some("src/Example.kt".to_string())).with_source(source);

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    let expected_separation = "    at src/Example.kt:1:1\n\n6 | val name = \"test\"\n";
    assert!(text.contains(expected_separation));
}

#[test]
fn test_full_mode_snippet_tab_indentation_caret_alignment() {
    let source = "\tval foo = 1;\n";
    let mut diag = make_diagnostic("standard:no-semi", Severity::Warning, 1, "Unnecessary ;");
    set_diagnostic_range(&mut diag, TextRange::new(1, 13, 1, 14));

    let report = LintReport::new(vec![diag], Some("src/Foo.kt".to_string())).with_source(source);

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("1 | \tval foo = 1;\n"));
    let expected_caret_line = "    \t           ^\n";
    assert!(text.contains(expected_caret_line));
    assert!(text.contains("warning: standard:no-semi: Unnecessary ;\n"));
    assert!(text.contains("    at src/Foo.kt:1:13\n"));
}

#[test]
fn test_linter_report_renders_full_snippet_end_to_end() {
    let linter = lint::Linter::standard();
    let code = "fun main() {\n    val a = 1; ; val b = 2\n}\n";
    let report = linter.lint_str_with_path(code, "src/Main.kt");
    assert!(report.source().is_some());

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("2 |     val a = 1; ; val b = 2\n"));
    assert!(text.contains("warning: standard:no-semi:"));
    assert!(text.contains("    at src/Main.kt:2:"));
}

#[test]
fn test_full_reports_mixed_snippet_and_compact_separation() {
    let source = "val a = 1;\n";
    let snippet_report = LintReport::new(
        vec![make_diagnostic(
            "standard:no-semi",
            Severity::Warning,
            1,
            "Semi",
        )],
        Some("src/Snippet.kt".to_string()),
    )
    .with_source(source);

    let compact_report = LintReport::new(
        vec![
            make_diagnostic("standard:no-semi", Severity::Warning, 1, "Compact 1"),
            make_diagnostic("standard:max-line-length", Severity::Error, 2, "Compact 2"),
        ],
        Some("src/Compact.kt".to_string()),
    );

    let mut output = Vec::new();
    render_smart_lint_reports(
        &mut output,
        DisplayTier::Full,
        &[snippet_report, compact_report],
        false,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("    at src/Snippet.kt:1:1\n\nsrc/Compact.kt:1:1:"));
    assert!(text.contains(
        "src/Compact.kt:1:1: [standard:no-semi] Compact 1 (warning)\nsrc/Compact.kt:2:1: [standard:max-line-length] Compact 2 (error)"
    ));
}

#[test]
fn test_full_mode_snippet_multi_target_separator() {
    let source = "fun compute() {\n    val a = 1\n    val b = 2\n}\n";
    let range1 = TextRange::new(1, 5, 1, 12);
    let range2 = TextRange::new(4, 1, 4, 2);
    let diag = Diagnostic {
        rule_id: SmolStr::new("standard:long-function"),
        severity: Severity::Warning,
        check_range: range1,
        report_ranges: smallvec::smallvec![range1, range2],
        message: "Function is too long".to_string(),
    };

    let report = LintReport::new(vec![diag], Some("src/Test.kt".to_string())).with_source(source);

    let mut output = Vec::new();
    render_smart_lint_reports(
        &mut output,
        DisplayTier::Full,
        std::slice::from_ref(&report),
        false,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("1 | fun compute() {\n"));
    assert!(text.contains("        ^\n...\n"));
    assert!(text.contains("4 | }\n"));
    assert!(text.contains("    ^\n"));
    assert!(text.contains("warning: standard:long-function: Function is too long\n"));
    assert!(text.contains("    at src/Test.kt:1:5\n    at src/Test.kt:4:1\n"));
}
