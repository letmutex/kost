mod ktfmt;

use format::FormattingOptions;
use ktfmt::runner::{SkippedInput, run_test_group};

fn assert_test_group(sub_dir: &str, options: &FormattingOptions, warnings: &[SkippedInput]) {
    let result = run_test_group(sub_dir, options, warnings);
    match result {
        Ok(summary) => {
            assert!(summary.total > 0, "No test fixtures found in '{sub_dir}'");
        }
        Err(failures) => {
            for f in failures.iter() {
                eprintln!("FAIL: {} ({})", f.input_path.display(), f.phase);
                eprintln!("=== EXPECTED ===\n{}", f.expected);
                eprintln!("=== ACTUAL ===\n{}", f.actual);
            }
            let first_failure = &failures[0];
            panic!(
                "{} fixtures failed in '{sub_dir}'. First failure:\n{first_failure}",
                failures.len()
            );
        }
    }
}

static ANNOTATION_SKIPPED: &[SkippedInput] = &[
    SkippedInput::new(
        "expressions.input",
        "unsupported syntax: annotations on expressions",
    ),
    SkippedInput::new(
        "expressions2.input",
        "unsupported syntax: annotations on expressions",
    ),
];

static TYPE_SKIPPED: &[SkippedInput] = &[
    SkippedInput::new(
        "intersections.input",
        "unsupported syntax: intersection types",
    ),
    SkippedInput::new(
        "nullableTypes.input",
        "unsupported syntax: parenthesized nullable types",
    ),
];

static GOOGLE_SKIPPED: &[SkippedInput] = &[
    SkippedInput::new(
        "ifWithMaxWidthCondition.input",
        "invalid syntax: assignment in expression",
    ),
    SkippedInput::new(
        "whenWithMaxWidthCondition.input",
        "invalid syntax: assignment in expression",
    ),
    SkippedInput::new(
        "whileMaxWidthCondition.input",
        "invalid syntax: assignment in expression",
    ),
];

static NEW_CODESTYLE_SKIPPED: &[SkippedInput] = &[
    SkippedInput::new(
        "AnnotationOnExpression.input",
        "unsupported syntax: annotations on expressions",
    ),
    SkippedInput::new(
        "AnnotationOnExpressionFullWidth.input",
        "unsupported syntax: annotations on expressions",
    ),
    SkippedInput::new(
        "DeclarationAnnotations.input",
        "unsupported syntax: declaration annotations",
    ),
    SkippedInput::new(
        "UseSiteTargets.input",
        "unsupported syntax: use-site targets on uninitialized property",
    ),
];

#[test]
fn test_format_annotation_cases() {
    assert_test_group(
        "format/annotation",
        &FormattingOptions::meta(),
        ANNOTATION_SKIPPED,
    );
}

#[test]
fn test_format_binary_cases() {
    assert_test_group("format/binary", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_call_cases() {
    assert_test_group("format/call", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_class_cases() {
    assert_test_group("format/class", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_comment_cases() {
    assert_test_group("format/comment", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_enums_cases() {
    assert_test_group("format/enums", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_function_cases() {
    assert_test_group("format/function", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_if_cases() {
    assert_test_group("format/if", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_import_cases() {
    assert_test_group("format/import", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_kdoc_cases() {
    assert_test_group("format/kdoc", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_lambda_cases() {
    assert_test_group("format/lambda", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_misc_cases() {
    assert_test_group("format/misc", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_property_cases() {
    assert_test_group("format/property", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_string_cases() {
    assert_test_group("format/string", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_format_type_cases() {
    assert_test_group("format/type", &FormattingOptions::meta(), TYPE_SKIPPED);
}

#[test]
fn test_format_when_cases() {
    assert_test_group("format/when", &FormattingOptions::meta(), &[]);
}

#[test]
fn test_google_format_cases() {
    assert_test_group("google", &FormattingOptions::google(), GOOGLE_SKIPPED);
}

#[test]
fn test_kotlinlang_format_cases() {
    assert_test_group("kotlinlang", &FormattingOptions::kotlinlang(), &[]);
}

#[test]
fn test_new_codestyle_format_cases() {
    assert_test_group(
        "new_codestyle",
        &FormattingOptions::meta(),
        NEW_CODESTYLE_SKIPPED,
    );
}

type ProgressGroup = (&'static str, FormattingOptions, &'static [SkippedInput]);

fn standard_groups() -> Vec<ProgressGroup> {
    vec![
        (
            "format/annotation",
            FormattingOptions::meta(),
            ANNOTATION_SKIPPED,
        ),
        ("format/binary", FormattingOptions::meta(), &[]),
        ("format/call", FormattingOptions::meta(), &[]),
        ("format/class", FormattingOptions::meta(), &[]),
        ("format/comment", FormattingOptions::meta(), &[]),
        ("format/enums", FormattingOptions::meta(), &[]),
        ("format/function", FormattingOptions::meta(), &[]),
        ("format/if", FormattingOptions::meta(), &[]),
        ("format/import", FormattingOptions::meta(), &[]),
        ("format/kdoc", FormattingOptions::meta(), &[]),
        ("format/lambda", FormattingOptions::meta(), &[]),
        ("format/misc", FormattingOptions::meta(), &[]),
        ("format/property", FormattingOptions::meta(), &[]),
        ("format/string", FormattingOptions::meta(), &[]),
        ("format/type", FormattingOptions::meta(), TYPE_SKIPPED),
        ("format/when", FormattingOptions::meta(), &[]),
    ]
}

fn style_preset_groups() -> Vec<ProgressGroup> {
    vec![
        ("google", FormattingOptions::google(), GOOGLE_SKIPPED),
        ("kotlinlang", FormattingOptions::kotlinlang(), &[]),
        (
            "new_codestyle",
            FormattingOptions::meta(),
            NEW_CODESTYLE_SKIPPED,
        ),
    ]
}

fn progress_groups() -> Vec<ProgressGroup> {
    let mut groups = standard_groups();
    groups.extend(style_preset_groups());
    groups
}

#[test]
#[ignore]
fn analyze_progress() {
    let mut total_cases = 0;
    let mut total_passed = 0;
    let mut total_skipped = 0;
    let mut total_failed = 0;

    for (sub_dir, options, warnings) in progress_groups() {
        let fixtures = ktfmt::runner::discover_input_fixtures(Some(sub_dir));
        let count = fixtures.len();
        total_cases += count;
        match run_test_group(sub_dir, &options, warnings) {
            Ok(summary) => {
                total_passed += summary.passed;
                total_skipped += summary.skipped;
                println!(
                    "[PASS] {sub_dir}: {}/{} passed ({} skipped)",
                    summary.passed, summary.total, summary.skipped
                );
            }
            Err(failures) => {
                let failed_count = failures.len();
                total_failed += failed_count;
                let passed = count - failed_count;
                println!("[FAIL] {sub_dir}: {passed}/{count} passed ({failed_count} failed)");
            }
        }
    }
    println!("\n=== PROGRESS SUMMARY ===");
    println!("Total fixtures: {total_cases}");
    println!("Passed: {total_passed}");
    println!("Skipped: {total_skipped}");
    println!("Failed: {total_failed}");
}
