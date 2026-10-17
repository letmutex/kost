#![allow(dead_code)]

use std::path::{Path, PathBuf};

use lint::config::ConfigLoader;
use lint::registry::RuleRegistry;
use lint::types::Diagnostic;
use lint::walker::lint_source_with_path;

const DEFAULT_RUNNER_YAML: &str = "lint:\n  includes:\n    - standard\n    - statistics\n";

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

/// Returns the root path to lint test cases directory.
pub fn cases_root_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LINT_CASES_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases")
}

/// Recursively discovers all Kotlin fixture paths under a sub-directory.
pub fn discover_case_fixtures(sub_dir: Option<&str>) -> Vec<PathBuf> {
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
        } else if is_fixture_file(&path) {
            accumulator.push(path);
        }
    }
}

fn is_fixture_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("kt")
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

fn resolve_rules_path(input_path: &Path) -> Option<PathBuf> {
    let parent = input_path.parent()?;
    let stem = input_path.file_stem()?.to_str()?;
    let file_name = input_path.file_name()?.to_str()?;

    let candidate_stem = parent.join(lint::concat_str!(stem, ".rules.yaml"));
    if candidate_stem.is_file() {
        return Some(candidate_stem);
    }

    let candidate_full = parent.join(lint::concat_str!(file_name, ".rules.yaml"));
    if candidate_full.is_file() {
        return Some(candidate_full);
    }

    None
}

fn resolve_output_path(input_path: &Path) -> Option<PathBuf> {
    let parent = input_path.parent()?;
    let stem = input_path.file_stem()?.to_str()?;
    let file_name = input_path.file_name()?.to_str()?;

    let candidate_stem = parent.join(lint::concat_str!(stem, ".output"));
    if candidate_stem.is_file() {
        return Some(candidate_stem);
    }

    let candidate_full = parent.join(lint::concat_str!(file_name, ".output"));
    if candidate_full.is_file() {
        return Some(candidate_full);
    }

    None
}

/// Formats a single diagnostic in canonical string line format.
pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
    let line_str = diagnostic.range.start_line.to_string();
    let col_str = diagnostic.range.start_col.to_string();
    lint::concat_str!(
        &line_str,
        ":",
        &col_str,
        ": [",
        diagnostic.severity.as_str(),
        "] ",
        &diagnostic.rule_id,
        ": ",
        &diagnostic.message
    )
}

/// Formats a list of diagnostics deterministically sorted by position and rule ID.
pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut sorted = diagnostics.to_vec();
    sorted.sort_by(|left, right| {
        left.range
            .start_line
            .cmp(&right.range.start_line)
            .then_with(|| left.range.start_col.cmp(&right.range.start_col))
            .then_with(|| left.range.end_line.cmp(&right.range.end_line))
            .then_with(|| left.range.end_col.cmp(&right.range.end_col))
            .then_with(|| left.rule_id.cmp(&right.rule_id))
            .then_with(|| left.message.cmp(&right.message))
    });

    let mut formatted_lines = Vec::with_capacity(sorted.len());
    for diagnostic in &sorted {
        formatted_lines.push(format_diagnostic(diagnostic));
    }
    formatted_lines.join("\n")
}

fn load_case_registry(input_path: &Path) -> Result<RuleRegistry, CaseFailure> {
    match resolve_rules_path(input_path) {
        Some(rules_path) => ConfigLoader::load_file(&rules_path).map_err(|error| CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "config_load",
            expected: String::new(),
            actual: error.to_string(),
        }),
        None => ConfigLoader::load_string(DEFAULT_RUNNER_YAML, "default.yaml").map_err(|error| {
            CaseFailure {
                input_path: input_path.to_path_buf(),
                phase: "default_config_load",
                expected: String::new(),
                actual: error.to_string(),
            }
        }),
    }
}

fn read_expected_output(input_path: &Path) -> Result<String, CaseFailure> {
    let output_path = resolve_output_path(input_path);
    match output_path {
        Some(path) => {
            let content = std::fs::read_to_string(&path).map_err(|error| CaseFailure {
                input_path: input_path.to_path_buf(),
                phase: "read_output",
                expected: String::new(),
                actual: error.to_string(),
            })?;
            Ok(content.replace("\r\n", "\n").trim_end().to_string())
        }
        None => Ok(String::new()),
    }
}

/// Runs a single test case asserting detected violations against `.output`.
pub fn run_test_case(
    input_path: &Path,
    skip_warning: Option<&str>,
) -> Result<CaseOutcome, CaseFailure> {
    if let Some(reason) = skip_warning {
        return Ok(CaseOutcome::Skipped(reason.to_string()));
    }

    let input = std::fs::read_to_string(input_path).map_err(|error| CaseFailure {
        input_path: input_path.to_path_buf(),
        phase: "read_input",
        expected: String::new(),
        actual: error.to_string(),
    })?;

    let registry = load_case_registry(input_path)?;

    let normalized_input = if input.contains('\r') {
        input.replace("\r\n", "\n")
    } else {
        input
    };

    let needs_file_path = matches!(
        input_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str()),
        Some("filename" | "no_empty_file")
    );
    let file_path = if needs_file_path {
        input_path.to_str()
    } else {
        None
    };

    let diagnostics = lint_source_with_path(normalized_input.as_bytes(), file_path, &registry);
    let actual_formatted = format_diagnostics(&diagnostics);
    let actual_normalized = actual_formatted
        .replace("\r\n", "\n")
        .trim_end()
        .to_string();

    let expected_normalized = read_expected_output(input_path)?;

    if actual_normalized != expected_normalized {
        return Err(CaseFailure {
            input_path: input_path.to_path_buf(),
            phase: "diagnostic_mismatch",
            expected: expected_normalized,
            actual: actual_normalized,
        });
    }

    Ok(CaseOutcome::Passed)
}

/// Runs all test fixtures in a given sub-directory with aggregated failure reporting.
pub fn run_test_group(
    sub_dir: &str,
    warnings: &[SkippedInput],
) -> Result<GroupSummary, Vec<CaseFailure>> {
    let fixtures = discover_case_fixtures(Some(sub_dir));
    let mut failures = Vec::new();
    let mut passed = 0;
    let mut skipped = 0;

    for fixture in &fixtures {
        let warning = find_warning(fixture, warnings);
        match run_test_case(fixture, warning) {
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
