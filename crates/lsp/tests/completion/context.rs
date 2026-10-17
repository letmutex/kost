use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_completion_disabled_in_line_and_block_comments() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TestComments.kt").unwrap();
    let file_code = r#"
package com.example

class User(val name: String)

fun test(user: User) {
    // user.
    /* user. */
    /**
     * user.
     */
}
"#;
    index.index_source(file_url.clone(), file_code);

    let line_comment_comps = find_completions(&index, &file_url, Position::new(6, 11), file_code);
    assert!(
        line_comment_comps.is_empty(),
        "Completion should be disabled inside line comments"
    );

    let block_comment_comps = find_completions(&index, &file_url, Position::new(7, 11), file_code);
    assert!(
        block_comment_comps.is_empty(),
        "Completion should be disabled inside block comments"
    );

    let kdoc_comps = find_completions(&index, &file_url, Position::new(9, 11), file_code);
    assert!(
        kdoc_comps.is_empty(),
        "Completion should be disabled inside KDoc comments"
    );
}

#[test]
fn test_completion_disabled_in_plain_strings() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TestStrings.kt").unwrap();
    let file_code = r#"
package com.example

class User(val name: String)

fun test(user: User) {
    val s1 = "user.name"
    val s2 = """
        user.name
    """
}
"#;
    index.index_source(file_url.clone(), file_code);

    let str_comps = find_completions(&index, &file_url, Position::new(6, 18), file_code);
    assert!(
        str_comps.is_empty(),
        "Completion should be disabled inside double-quoted string literals"
    );

    let triple_str_comps = find_completions(&index, &file_url, Position::new(8, 13), file_code);
    assert!(
        triple_str_comps.is_empty(),
        "Completion should be disabled inside triple-quoted string literals"
    );
}

#[test]
fn test_completion_enabled_in_string_template_interpolation() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TestInterpolation.kt").unwrap();
    let file_code = r#"
package com.example

class User(val id: String, val name: String)

fun test(user: User) {
    val s1 = "Hello ${user.}"
    val s2 = """
        Result: ${user.}
    """
    val s3 = "${ /* user. */ 123 }"
}
"#;
    index.index_source(file_url.clone(), file_code);

    let interp_comps = find_completions(&index, &file_url, Position::new(6, 27), file_code);
    let interp_labels: Vec<String> = interp_comps.into_iter().map(|c| c.label).collect();
    assert!(
        interp_labels.contains(&"id".to_string()),
        "Should contain 'id' inside string template interpolation"
    );
    assert!(
        interp_labels.contains(&"name".to_string()),
        "Should contain 'name' inside string template interpolation"
    );

    let triple_interp_comps = find_completions(&index, &file_url, Position::new(8, 23), file_code);
    let triple_labels: Vec<String> = triple_interp_comps.into_iter().map(|c| c.label).collect();
    assert!(
        triple_labels.contains(&"id".to_string()),
        "Should contain 'id' inside triple-quoted template interpolation"
    );
    assert!(
        triple_labels.contains(&"name".to_string()),
        "Should contain 'name' inside triple-quoted template interpolation"
    );

    let comment_in_interp = find_completions(&index, &file_url, Position::new(10, 24), file_code);
    assert!(
        comment_in_interp.is_empty(),
        "Completion should be disabled inside comments within string templates"
    );
}

#[test]
fn test_completion_disabled_for_raw_numbers() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/TestNumbers.kt").unwrap();
    let file_code = r#"
package com.example

class User1(val id: String)

fun test() {
    val a = 42
    val b = 0xFF
    val c = 100L
    val d = 123.
    val e = 3.14.
    val u = User1("1")
    val f = user1
}
"#;
    index.index_source(file_url.clone(), file_code);

    let comps_42 = find_completions(&index, &file_url, Position::new(6, 14), file_code);
    assert!(
        comps_42.is_empty(),
        "Scope completion should be disabled for raw integer '42'"
    );

    let comps_hex = find_completions(&index, &file_url, Position::new(7, 16), file_code);
    assert!(
        comps_hex.is_empty(),
        "Scope completion should be disabled for hex literal '0xFF'"
    );

    let comps_long = find_completions(&index, &file_url, Position::new(8, 16), file_code);
    assert!(
        comps_long.is_empty(),
        "Scope completion should be disabled for long literal '100L'"
    );

    let comps_dot = find_completions(&index, &file_url, Position::new(9, 16), file_code);
    assert!(
        comps_dot.is_empty(),
        "Member completion should be disabled for bare number '123.'"
    );

    let comps_float = find_completions(&index, &file_url, Position::new(10, 17), file_code);
    assert!(
        comps_float.is_empty(),
        "Member completion should be disabled for bare float '3.14.'"
    );

    let comps_user = find_completions(&index, &file_url, Position::new(12, 17), file_code);
    let user_labels: Vec<String> = comps_user.into_iter().map(|c| c.label).collect();
    assert!(
        user_labels.contains(&"User1".to_string()),
        "Completion should work normally for identifiers like 'user1'"
    );
}
