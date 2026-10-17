use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_member_completion_data_class_synthetic_copy() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DataClassCopyTest.kt").unwrap();
    let code = r#"
package com.example.dataclass

data class UserProfile(val id: String, val displayName: String)

fun test(profile: UserProfile) {
    profile.
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(6, 12);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"id".to_string()),
        "profile. should offer property 'id'"
    );
    assert!(
        labels.contains(&"displayName".to_string()),
        "profile. should offer property 'displayName'"
    );
    assert!(
        labels.contains(&"copy".to_string()),
        "profile. should offer synthetic 'copy' method for Kotlin data class"
    );
}

#[test]
fn test_member_completion_enum_synthetic_entries_and_values() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/EnumSyntheticTest.kt").unwrap();
    let code = r#"
package com.example.enums

enum class Severity {
    LOW,
    MEDIUM,
    HIGH
}

fun test() {
    Severity.
}
"#;
    index.index_source(url.clone(), code);

    let pos_dot = Position::new(10, 13);
    let comps = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"LOW".to_string()),
        "Severity. should offer entry 'LOW'"
    );
    assert!(
        labels.contains(&"entries".to_string()),
        "Severity. should offer synthetic Kotlin enum property 'entries'"
    );
    assert!(
        labels.contains(&"values".to_string()),
        "Severity. should offer synthetic Kotlin enum method 'values'"
    );
}
