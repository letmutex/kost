use format::create_diff;

#[test]
fn test_create_diff_identical_strings_returns_none() {
    let text = "fun main() {\n    println(\"hello\")\n}\n";
    assert_eq!(create_diff(text, text, "Test.kt", false), None);
}

#[test]
fn test_create_diff_modification_outputs_unified_diff() {
    let original = "fun main() {\n  val x = 1\n}\n";
    let formatted = "fun main() {\n    val x = 1\n}\n";
    let diff = create_diff(original, formatted, "src/Main.kt", false).unwrap();

    assert!(diff.starts_with("src/Main.kt -1 +1\n@@ -1,3 +1,3 @@"));
    assert!(diff.contains("-  val x = 1\n"));
    assert!(diff.contains("+    val x = 1\n"));
}

#[test]
fn test_create_diff_insertions_and_deletions() {
    let original = "line1\nline2\nline3\n";
    let formatted = "line1\nline2_modified\nline3\nline4\n";
    let diff = create_diff(original, formatted, "File.kt", false).unwrap();

    assert!(diff.starts_with("File.kt -1 +2\n"));
    assert!(diff.contains("-line2\n"));
    assert!(diff.contains("+line2_modified\n"));
    assert!(diff.contains("+line4\n"));
}

#[test]
fn test_create_diff_trailing_newline_difference_outputs_hunk() {
    let original = "fun main() {}";
    let formatted = "fun main() {}\n";
    let diff = create_diff(original, formatted, "Main.kt", false).unwrap();

    assert!(diff.starts_with("Main.kt -1 +2\n"));
    assert!(diff.contains("-fun main() {}\n"));
    assert!(diff.contains("+fun main() {}\n"));
    assert!(diff.contains("2 +\n"));
}

#[test]
fn test_create_diff_empty_insertion_has_posix_zero_line_number() {
    let original = "";
    let formatted = "val x = 1\n";
    let diff = create_diff(original, formatted, "New.kt", false).unwrap();

    assert!(diff.starts_with("New.kt +2\n"));
    assert!(diff.contains("@@ -0,0 +1 @@\n"));
    assert!(diff.contains("+val x = 1\n"));
}

#[test]
fn test_create_diff_multiline_hunk_emits_added_eof_newline_marker() {
    let original = "fun main() {\n  val x = 1";
    let formatted = "fun main() {\n    val x = 1\n}\n";
    let diff = create_diff(original, formatted, "Main.kt", false).unwrap();

    assert!(!diff.contains("\\ No newline at end of file"));
    assert!(diff.contains("+\n"));
}

#[test]
fn test_create_diff_trailing_newline_addition_with_trailing_context_emits_diff() {
    let original = "val x = 1\nval y = 2";
    let formatted = "val x = 10\nval y = 2\n";
    let diff = create_diff(original, formatted, "File.kt", false).unwrap();
    assert!(diff.contains("-val y = 2\n"));
    assert!(diff.contains("+val x = 10\n"));
    assert!(diff.contains("+val y = 2\n"));
    assert!(diff.contains("+\n"));
}

#[test]
fn test_create_diff_crlf_line_ending_preserves_carriage_return() {
    let original = "val x = 1\r\n";
    let formatted = "val x = 1\n";
    let diff = create_diff(original, formatted, "File.kt", false).unwrap();
    assert!(diff.contains("-val x = 1\r\n"));
    assert!(diff.contains("+val x = 1\n"));
}

#[test]
fn test_create_diff_line_number_gutter_alignment() {
    let original = "fun main() {\n  val x = 1\n}\n";
    let formatted = "fun main() {\n    val x = 1\n}\n";
    let diff = create_diff(original, formatted, "Main.kt", false).unwrap();

    let expected =
        "Main.kt -1 +1\n@@ -1,3 +1,3 @@\n1  fun main() {\n2 -  val x = 1\n2 +    val x = 1\n3  }\n";
    assert_eq!(diff, expected);
}

#[test]
fn test_create_diff_deletions_only_header() {
    let original = "line1\nline2\n";
    let formatted = "line1\n";
    let diff = create_diff(original, formatted, "File.kt", false).unwrap();

    assert!(diff.starts_with("File.kt -1\n"));
    assert!(diff.contains("2 -line2\n"));
}

#[test]
fn test_create_diff_consecutive_insertions_omits_subsequent_line_numbers() {
    let original = "fun main() {\n}\n";
    let formatted = "fun main() {\n    val a = 1\n    val b = 2\n}\n";
    let diff = create_diff(original, formatted, "Main.kt", false).unwrap();

    let expected =
        "Main.kt +2\n@@ -1,2 +1,4 @@\n1  fun main() {\n2 +    val a = 1\n  +    val b = 2\n2  }\n";
    assert_eq!(diff, expected);
}
