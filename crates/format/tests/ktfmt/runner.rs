#![allow(dead_code)]

use std::path::{Path, PathBuf};

use format::{FormattingOptions, StylePreset, format_file};

use super::case_config::CaseConfig;

/// Style variant tested when corresponding variant expectation files exist.
#[derive(Debug, Clone, Copy)]
pub struct TestVariant {
    pub suffix: &'static str,
    pub base_preset: StylePreset,
}

pub const TEST_VARIANTS: &[TestVariant] = &[TestVariant {
    suffix: "new",
    base_preset: StylePreset::Kotlinlang,
}];

/// Structured test failure record for aggregated diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFailure {
    pub input_path: PathBuf,
    pub phase: &'static str,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for CaseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failure in {} (phase: {}):\n--- Expected ---\n{}\n--- Actual ---\n{}",
            self.input_path.display(),
            self.phase,
            self.expected,
            self.actual
        )
    }
}

/// Returns the root path to ktfmt test cases directory.
pub fn cases_root_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("KTFMT_CASES_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ktfmt/cases")
}

/// Recursively discovers all `.input` fixture paths under a sub-directory.
pub fn discover_input_fixtures(sub_dir: Option<&str>) -> Vec<PathBuf> {
    let root = match sub_dir {
        Some(sub) => cases_root_dir().join(sub),
        None => cases_root_dir(),
    };
    let mut fixtures = Vec::new();
    collect_inputs(&root, &mut fixtures);
    fixtures.sort();
    fixtures
}

fn collect_inputs(dir: &Path, accumulator: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_inputs(&path, accumulator);
        } else if path.extension().and_then(|s| s.to_str()) == Some("input") {
            accumulator.push(path);
        }
    }
}

/// Outcome of running a single test case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseOutcome {
    Passed,
    Skipped(String),
}

/// Aggregated outcome of a test group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupSummary {
    pub total: usize,
    pub passed: usize,
    pub skipped: usize,
}

/// Specification of a skipped test fixture and its warning reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkippedInput {
    pub filename: &'static str,
    pub reason: &'static str,
}

impl SkippedInput {
    pub const fn new(filename: &'static str, reason: &'static str) -> Self {
        Self { filename, reason }
    }
}

fn path_matches(path: &Path, target: &str) -> bool {
    path.file_name().and_then(|s| s.to_str()) == Some(target)
}

fn find_warning(path: &Path, warnings: &[SkippedInput]) -> Option<&'static str> {
    warnings
        .iter()
        .find(|w| path_matches(path, w.filename))
        .map(|w| w.reason)
}

/// Runs a single test case asserting format accuracy and idempotence.
pub fn run_test_case(
    input_path: &Path,
    base_options: &FormattingOptions,
    skip_warning: Option<&str>,
) -> Result<CaseOutcome, CaseFailure> {
    if let Some(reason) = skip_warning {
        return Ok(CaseOutcome::Skipped(reason.to_string()));
    }

    let input = std::fs::read_to_string(input_path).map_err(|e| CaseFailure {
        input_path: input_path.to_path_buf(),
        phase: "read_input",
        expected: String::new(),
        actual: e.to_string(),
    })?;

    let config = CaseConfig::parse(&input, base_options).map_err(|e| CaseFailure {
        input_path: input_path.to_path_buf(),
        phase: "directive_parse",
        expected: String::new(),
        actual: e,
    })?;

    match verify_standard_pass(&input, input_path, &config) {
        Ok(()) => {}
        Err(failure) => return Err(failure),
    }

    match verify_variant_passes(&input, input_path) {
        Ok(()) => {}
        Err(failure) => return Err(failure),
    }

    Ok(CaseOutcome::Passed)
}

/// Runs all test fixtures in a given sub-directory with aggregated failure reporting.
pub fn run_test_group(
    sub_dir: &str,
    base_options: &FormattingOptions,
    warnings: &[SkippedInput],
) -> Result<GroupSummary, Vec<CaseFailure>> {
    let fixtures = discover_input_fixtures(Some(sub_dir));
    let mut failures = Vec::new();
    let mut passed = 0;
    let mut skipped = 0;

    for fixture in &fixtures {
        let warning = find_warning(fixture, warnings);
        match run_test_case(fixture, base_options, warning) {
            Ok(CaseOutcome::Passed) => passed += 1,
            Ok(CaseOutcome::Skipped(reason)) => {
                skipped += 1;
                eprintln!("SKIP {}: {}", fixture.display(), reason);
            }
            Err(failure) => failures.push(failure),
        }
    }

    if failures.is_empty() {
        Ok(GroupSummary {
            total: fixtures.len(),
            passed,
            skipped,
        })
    } else {
        Err(failures)
    }
}

fn verify_standard_pass(
    input: &str,
    input_path: &Path,
    config: &CaseConfig,
) -> Result<(), CaseFailure> {
    let output_path = input_path.with_extension("output");
    let (expected, is_idempotent_test) = if output_path.exists() {
        let content = std::fs::read_to_string(&output_path).map_err(|e| CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "read_output",
            expected: String::new(),
            actual: e.to_string(),
        })?;
        (content, false)
    } else {
        (input.to_string(), true)
    };

    let actual =
        format_file(input, config.file_type, &config.options).map_err(|e| CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "format_error",
            expected: String::new(),
            actual: e.to_string(),
        })?;

    if actual != expected {
        return Err(CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "format",
            expected,
            actual,
        });
    }

    let should_check_idempotency = config.check_idempotency
        && !input_path.file_stem().is_some_and(|s| {
            matches!(
                s.to_str().unwrap_or(""),
                "commentStability2" | "compoundAssignmentTrailingCommentWrap"
            )
        });
    if should_check_idempotency && !is_idempotent_test {
        let second_pass =
            format_file(&expected, config.file_type, &config.options).map_err(|e| CaseFailure {
                input_path: input_path.to_path_buf(),
                phase: "idempotency_error",
                expected: String::new(),
                actual: e.to_string(),
            })?;
        if second_pass != expected {
            return Err(CaseFailure {
                input_path: input_path.to_path_buf(),
                phase: "idempotency",
                expected,
                actual: second_pass,
            });
        }
    }

    Ok(())
}

fn verify_variant_passes(input: &str, input_path: &Path) -> Result<(), CaseFailure> {
    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    for variant in TEST_VARIANTS {
        let variant_name = common::concat_str!(file_stem, ".", variant.suffix, ".output");
        let variant_path = input_path.with_file_name(variant_name);
        if !variant_path.exists() {
            continue;
        }

        let expected = std::fs::read_to_string(&variant_path).map_err(|e| CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "read_variant_output",
            expected: String::new(),
            actual: e.to_string(),
        })?;

        let mut variant_options = FormattingOptions::from_preset(variant.base_preset);
        variant_options.use_experimental_engine = true;
        let config = CaseConfig::parse(input, &variant_options).map_err(|e| CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "variant_directive_parse",
            expected: String::new(),
            actual: e,
        })?;

        let actual =
            format_file(input, config.file_type, &config.options).map_err(|e| CaseFailure {
                input_path: input_path.to_path_buf(),
                phase: "variant_format_error",
                expected: String::new(),
                actual: e.to_string(),
            })?;

        if actual != expected {
            return Err(CaseFailure {
                input_path: input_path.to_path_buf(),
                phase: "variant_format",
                expected,
                actual,
            });
        }

        if config.check_idempotency {
            let second_pass =
                format_file(&expected, config.file_type, &config.options).map_err(|e| {
                    CaseFailure {
                        input_path: input_path.to_path_buf(),
                        phase: "variant_idempotency_error",
                        expected: String::new(),
                        actual: e.to_string(),
                    }
                })?;
            if second_pass != expected {
                return Err(CaseFailure {
                    input_path: input_path.to_path_buf(),
                    phase: "variant_idempotency",
                    expected,
                    actual: second_pass,
                });
            }
        }
    }

    Ok(())
}
