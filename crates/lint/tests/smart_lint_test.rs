use cli::PrintMode;
use lint::LintReport;
use lint::smart_lint::{
    DisplayTier, render_diagnostic, render_smart_lint_reports, resolve_display_tier,
};
use lint::types::{Diagnostic, Severity, TextRange};
use smol_str::SmolStr;

fn make_diagnostic(rule_id: &str, severity: Severity, line: u32, message: &str) -> Diagnostic {
    Diagnostic {
        rule_id: SmolStr::new(rule_id),
        severity,
        range: TextRange::new(line, 1, line, 10),
        message: message.to_string(),
    }
}

#[test]
fn test_display_tier_resolution_auto_thresholds() {
    assert_eq!(resolve_display_tier(PrintMode::Auto, 0), DisplayTier::Full);
    assert_eq!(resolve_display_tier(PrintMode::Auto, 5), DisplayTier::Full);
    assert_eq!(resolve_display_tier(PrintMode::Auto, 10), DisplayTier::Full);
    assert_eq!(
        resolve_display_tier(PrintMode::Auto, 11),
        DisplayTier::Truncated
    );
    assert_eq!(
        resolve_display_tier(PrintMode::Auto, 20),
        DisplayTier::Truncated
    );
    assert_eq!(
        resolve_display_tier(PrintMode::Auto, 21),
        DisplayTier::Collapsed
    );
    assert_eq!(
        resolve_display_tier(PrintMode::Auto, 100),
        DisplayTier::Collapsed
    );
}

#[test]
fn test_display_tier_resolution_explicit_overrides() {
    assert_eq!(resolve_display_tier(PrintMode::Full, 50), DisplayTier::Full);
    assert_eq!(
        resolve_display_tier(PrintMode::Truncated, 3),
        DisplayTier::Truncated
    );
    assert_eq!(
        resolve_display_tier(PrintMode::Collapsed, 1),
        DisplayTier::Collapsed
    );
}

#[test]
fn test_smart_lint_full_tier_renders_all_diagnostics() {
    let diagnostics = vec![
        make_diagnostic("standard:no-semi", Severity::Warning, 1, "Unnecessary ;"),
        make_diagnostic(
            "standard:max-line-length",
            Severity::Error,
            2,
            "Line too long",
        ),
    ];
    let report = LintReport::new(diagnostics, Some("src/Main.kt".to_string()));

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Full, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("src/Main.kt:1:1: [standard:no-semi] Unnecessary ; (warning)"));
    assert!(text.contains("src/Main.kt:2:1: [standard:max-line-length] Line too long (error)"));
    assert!(!text.contains("truncated"));
    assert!(!text.contains("collapsed"));
}

#[test]
fn test_smart_lint_truncated_tier_limits_to_twenty_and_appends_footer() {
    let mut diagnostics = Vec::new();
    for i in 1..=25 {
        diagnostics.push(make_diagnostic(
            "standard:no-semi",
            Severity::Warning,
            i,
            "Unnecessary ;",
        ));
    }
    let report = LintReport::new(diagnostics, Some("src/Big.kt".to_string()));

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Truncated, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("src/Big.kt:1:1: [standard:no-semi] Unnecessary ; (warning)"));
    assert!(text.contains("src/Big.kt:20:1: [standard:no-semi] Unnecessary ; (warning)"));
    assert!(!text.contains("src/Big.kt:21:1:"));
    assert!(
        text.contains(
            "lint diagnostics truncated (use 'kost lint src/Big.kt' to view full report)\n"
        )
    );
    assert!(!text.contains("..."));
}

#[test]
fn test_smart_lint_truncated_tier_under_limit_omits_footer() {
    let diagnostics = vec![make_diagnostic(
        "standard:no-semi",
        Severity::Warning,
        1,
        "Unnecessary ;",
    )];
    let report = LintReport::new(diagnostics, Some("src/Small.kt".to_string()));

    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Truncated, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("src/Small.kt:1:1: [standard:no-semi] Unnecessary ; (warning)"));
    assert!(!text.contains("truncated"));
}

#[test]
fn test_smart_lint_collapsed_tier_renders_counts_and_footer() {
    let report1 = LintReport::new(
        vec![
            make_diagnostic("rule:one", Severity::Error, 1, "Err 1"),
            make_diagnostic("rule:two", Severity::Warning, 2, "Warn 1"),
        ],
        Some("src/One.kt".to_string()),
    );
    let report2 = LintReport::new(
        vec![make_diagnostic(
            "rule:three",
            Severity::Warning,
            5,
            "Warn 2",
        )],
        Some("src/Two.kt".to_string()),
    );

    let mut output = Vec::new();
    render_smart_lint_reports(
        &mut output,
        DisplayTier::Collapsed,
        &[report1, report2],
        false,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("src/One.kt: 1 error, 1 warning\n"));
    assert!(text.contains("src/Two.kt: 1 warning\n"));
    assert!(
        text.contains(
            "lint collapsed for 2 files (use 'kost lint <filepath>' to view full report)\n"
        )
    );
    assert!(!text.contains("..."));
}

#[test]
fn test_smart_lint_collapsed_tier_empty_reports_emits_nothing() {
    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Collapsed, &[], false).unwrap();
    assert!(output.is_empty());
}

#[test]
fn test_smart_lint_collapsed_tier_zero_violations_emits_zero_issues() {
    let report = LintReport::new(Vec::new(), Some("src/Clean.kt".to_string()));
    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Collapsed, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("src/Clean.kt: 0 issues\n"));
    assert!(
        text.contains(
            "lint collapsed for 1 file (use 'kost lint <filepath>' to view full report)\n"
        )
    );
}

#[test]
fn test_smart_lint_collapsed_tier_plural_errors_and_singular_file() {
    let report = LintReport::new(
        vec![
            make_diagnostic("rule:err1", Severity::Error, 1, "E1"),
            make_diagnostic("rule:err2", Severity::Error, 2, "E2"),
        ],
        Some("src/Errors.kt".to_string()),
    );
    let mut output = Vec::new();
    render_smart_lint_reports(&mut output, DisplayTier::Collapsed, &[report], false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("src/Errors.kt: 2 errors\n"));
    assert!(
        text.contains(
            "lint collapsed for 1 file (use 'kost lint <filepath>' to view full report)\n"
        )
    );
}

#[test]
fn test_diagnostic_formatting_without_color() {
    let diag = make_diagnostic("standard:no-semi", Severity::Warning, 10, "Semi");
    let mut output = Vec::new();
    render_diagnostic(&mut output, &diag, Some("src/Foo.kt"), false).unwrap();
    let text = String::from_utf8(output).unwrap();

    assert_eq!(text, "src/Foo.kt:10:1: [standard:no-semi] Semi (warning)\n");
    assert!(!text.contains("\x1b"));
}
