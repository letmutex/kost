use cli::PrintMode;
use format::smart_diff::{
    DisplayTier, render_smart_diffs, resolve_display_tier, truncate_diff_to_lines,
};
use std::fmt::Write as _;

#[test]
fn test_resolve_display_tier_auto_thresholds() {
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
fn test_resolve_display_tier_explicit_override_modes() {
    assert_eq!(resolve_display_tier(PrintMode::Full, 50), DisplayTier::Full);
    assert_eq!(
        resolve_display_tier(PrintMode::Truncated, 1),
        DisplayTier::Truncated
    );
    assert_eq!(
        resolve_display_tier(PrintMode::Truncated, 50),
        DisplayTier::Truncated
    );
    assert_eq!(
        resolve_display_tier(PrintMode::Collapsed, 1),
        DisplayTier::Collapsed
    );
}

#[test]
fn test_truncate_diff_to_lines_preserves_under_limit() {
    let diff = "line1\nline2\nline3\n";
    assert_eq!(truncate_diff_to_lines(diff, 5), None);
    assert_eq!(truncate_diff_to_lines(diff, 3), None);
}

#[test]
fn test_truncate_diff_to_lines_cuts_at_limit() {
    let mut diff = String::new();
    for i in 1..=25 {
        let _ = writeln!(diff, "line{i}");
    }
    let (truncated, remaining) = truncate_diff_to_lines(&diff, 20).unwrap();
    assert_eq!(remaining, 5);
    assert!(truncated.ends_with("line20\n"));
    assert!(!truncated.contains("line21"));
}

#[test]
fn test_truncate_diff_to_lines_preserves_crlf() {
    let mut diff = String::new();
    for i in 1..=25 {
        let _ = write!(diff, "line{i}\r\n");
    }
    let (truncated, remaining) = truncate_diff_to_lines(&diff, 20).unwrap();
    assert_eq!(remaining, 5);
    assert!(truncated.ends_with("line20\r\n"));
}

#[test]
fn test_render_smart_diffs_truncated_omits_leading_dots() {
    let mut diff = String::new();
    for i in 1..=25 {
        let _ = writeln!(diff, "line{i}");
    }
    let files = [("File.kt", Some(diff.as_str()))];
    let mut output = Vec::new();
    render_smart_diffs(&mut output, DisplayTier::Truncated, &files, false).unwrap();
    let rendered = String::from_utf8(output).unwrap();

    assert!(!rendered.contains("..."));
    assert!(
        rendered.contains("diff truncated (use 'kost fmt --check File.kt' to view full diff)\n")
    );
}

#[test]
fn test_render_smart_diffs_collapsed_header_and_overall_footer() {
    let diff1 = "File1.kt -1 +1\n@@ -1 +1 @@\n-a\n+b\n";
    let diff2 = "File2.kt -1 +1\n@@ -1 +1 @@\n-c\n+d\n";
    let files = [("File1.kt", Some(diff1)), ("File2.kt", Some(diff2))];
    let mut output = Vec::new();
    render_smart_diffs(&mut output, DisplayTier::Collapsed, &files, false).unwrap();
    let rendered = String::from_utf8(output).unwrap();

    assert!(!rendered.contains("..."));
    assert!(rendered.contains("File1.kt -1 +1\n"));
    assert!(rendered.contains("File2.kt -1 +1\n"));
    assert!(!rendered.contains("@@ -1 +1 @@"));
    assert!(rendered.contains(
        "diff collapsed for 2 files (use 'kost fmt --check <filepath>' to view full diff)\n"
    ));
}

#[test]
fn test_truncate_diff_to_lines_zero_max_lines() {
    let diff = "line1\nline2\n";
    let (truncated, remaining) = truncate_diff_to_lines(diff, 0).unwrap();
    assert_eq!(truncated, "");
    assert_eq!(remaining, 2);

    let empty = "";
    assert_eq!(truncate_diff_to_lines(empty, 0), None);
}

#[test]
fn test_render_smart_diffs_fallback_for_none_diff() {
    let files = [("MissingDiff.kt", None)];
    let mut output = Vec::new();
    render_smart_diffs(&mut output, DisplayTier::Collapsed, &files, false).unwrap();
    let rendered = String::from_utf8(output).unwrap();

    assert!(rendered.contains("MissingDiff.kt\n"));
    assert!(!rendered.contains("---+++"));
    assert!(rendered.contains("diff collapsed for 1 file"));
}
