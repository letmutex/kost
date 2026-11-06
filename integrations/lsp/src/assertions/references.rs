use common::concat_str;
use lsp::features::find_references;
use std::path::Path;
use tower_lsp::lsp_types::{Location, Position};

use crate::config::{ExpectedReferenceLocation, ReferencesTest, strip_unc_prefix};
use crate::environment::WorkspaceEnvironment;

#[derive(Debug, Clone)]
struct NormalizedActualReference {
    relative_path: String,
    start_line: u32,
    start_col: u32,
    end_col: u32,
}

fn normalize_actual_references(
    locations: &[Location],
    root_path: &Path,
) -> Vec<NormalizedActualReference> {
    let clean_root = strip_unc_prefix(root_path);
    locations
        .iter()
        .map(|loc| {
            let relative_path = loc
                .uri
                .to_file_path()
                .ok()
                .map(|p| strip_unc_prefix(&p))
                .and_then(|abs_path| {
                    abs_path
                        .strip_prefix(&clean_root)
                        .ok()
                        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                })
                .unwrap_or_else(|| loc.uri.to_string());

            NormalizedActualReference {
                relative_path,
                start_line: loc.range.start.line,
                start_col: loc.range.start.character,
                end_col: loc.range.end.character,
            }
        })
        .collect()
}

fn find_missing_subset<'a>(
    expected_subset: &'a [ExpectedReferenceLocation],
    actual_references: &[NormalizedActualReference],
) -> Vec<&'a ExpectedReferenceLocation> {
    let mut missing = Vec::new();
    for expected in expected_subset {
        let expected_normalized_path = expected.file.replace('\\', "/");
        let matched = actual_references.iter().any(|actual| {
            actual
                .relative_path
                .eq_ignore_ascii_case(&expected_normalized_path)
                && actual.start_line == expected.line
                && (actual.start_col == expected.col
                    || (actual.start_col <= expected.col && expected.col <= actual.end_col))
        });

        if !matched {
            missing.push(expected);
        }
    }
    missing
}

fn format_missing_error(
    test_case: &ReferencesTest,
    actual_count: usize,
    missing_references: &[&ExpectedReferenceLocation],
    normalized_actual: &[NormalizedActualReference],
) -> String {
    let mut details = concat_str!(
        "Exact subset matching failed for '",
        &test_case.symbol,
        "' in ",
        &test_case.file,
        ". Found ",
        &actual_count.to_string(),
        " total references, but ",
        &missing_references.len().to_string(),
        " expected references were missing:\n"
    );

    for missing in missing_references {
        let line_str = missing.line.to_string();
        let col_str = missing.col.to_string();
        details.push_str(&concat_str!(
            "  - Missing: ",
            &missing.file,
            ":",
            &line_str,
            ":",
            &col_str,
            "\n"
        ));

        let missing_normalized = missing.file.replace('\\', "/");
        let same_file_refs: Vec<_> = normalized_actual
            .iter()
            .filter(|act| act.relative_path.eq_ignore_ascii_case(&missing_normalized))
            .collect();

        if !same_file_refs.is_empty() {
            details.push_str("    Actual occurrences found in that file:\n");
            for actual in same_file_refs {
                let actual_line_str = actual.start_line.to_string();
                let actual_col_str = actual.start_col.to_string();
                details.push_str(&concat_str!(
                    "      * line ",
                    &actual_line_str,
                    ", col ",
                    &actual_col_str,
                    "\n"
                ));
            }
        }
    }

    details
}

pub fn evaluate_references_test(
    environment: &WorkspaceEnvironment,
    test_case: &ReferencesTest,
) -> Result<(), String> {
    let file_url = environment.resolve_file_url(&test_case.file)?;
    let source_code = environment
        .get_file_source(&file_url)
        .ok_or_else(|| concat_str!("Source file not found in index: ", &test_case.file))?;

    let cursor_position = Position::new(test_case.position.line, test_case.position.col);
    let actual_locations = find_references(
        &environment.index,
        &file_url,
        &test_case.symbol,
        cursor_position,
        &source_code,
        test_case.include_declaration,
    );

    let normalized_actual = normalize_actual_references(&actual_locations, &environment.root_path);
    let missing_references = find_missing_subset(&test_case.expected_subset, &normalized_actual);

    if !missing_references.is_empty() {
        let error_msg = format_missing_error(
            test_case,
            actual_locations.len(),
            &missing_references,
            &normalized_actual,
        );
        return Err(error_msg);
    }

    Ok(())
}
