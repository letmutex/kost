use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_completion_member_reference_suggests_functions_and_properties() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberCompletion.kt").unwrap();
    let source = r#"
package com.example

class UserProfile {
    val displayName: String = ""
    fun fetchDetails(): String = ""
}

fun main() {
    val ref = UserProfile::
}
"#;
    index.index_source(url.clone(), source);

    // Cursor right after `UserProfile::` (line 9, col 27)
    let pos = Position::new(9, 27);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();

    assert!(
        labels.contains(&"displayName"),
        "Should suggest member property"
    );
    assert!(
        labels.contains(&"fetchDetails"),
        "Should suggest member function"
    );

    let fetch_item = completions
        .iter()
        .find(|item| item.label == "fetchDetails")
        .unwrap();

    if let Some(ref insert_text) = fetch_item.insert_text {
        assert_eq!(
            insert_text, "fetchDetails",
            "Member function reference must not contain call parentheses"
        );
    }
}

#[test]
fn test_completion_member_reference_suggests_class_literal() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ClassLiteralCompletion.kt").unwrap();
    let source = r#"
package com.example

class Config

fun main() {
    val ref = Config::cl
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `Config::cl` (line 6, col 24)
    let pos = Position::new(6, 24);
    let completions = find_completions(&index, &url, pos, source);

    let class_item = completions
        .iter()
        .find(|item| item.label == "class")
        .expect("class literal should be suggested");

    assert_eq!(class_item.insert_text.as_deref(), Some("class"));
}

#[test]
fn test_completion_extension_function_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionCompletion.kt").unwrap();
    let source = r#"
package com.example

class Book(val title: String)

fun Book.summarize(): String = title

fun main() {
    val ref = Book::sum
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `Book::sum` (line 8, col 23)
    let pos = Position::new(8, 23);
    let completions = find_completions(&index, &url, pos, source);

    let sum_item = completions
        .iter()
        .find(|item| item.label == "summarize")
        .expect("Extension function summarize should be suggested on Book::");

    if let Some(ref insert_text) = sum_item.insert_text {
        assert_eq!(
            insert_text, "summarize",
            "Extension function reference must not contain call parentheses"
        );
    }
}

#[test]
fn test_multiline_callable_reference_completion_with_trailing_colons() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultilineCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

class CalculationEngine {
    fun calculateValue(): Int = 42
}

fun main() {
    val ref = CalculationEngine::
        calc
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `calc` (line 9, col 12)
    let pos = Position::new(9, 12);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"calculateValue"),
        "Member function should be completed when receiver:: is on the previous line"
    );
}

#[test]
fn test_completion_numeric_receiver_callable_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NumericCallableCompletion.kt").unwrap();
    let source = r#"
package com.example

fun Int.customIntFormat(): String = ""

fun main() {
    val ref = 42::cust
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `42::cust` (line 6, col 22)
    let pos = Position::new(6, 22);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"customIntFormat"),
        "Extension function on Int should be suggested for 42::"
    );
}

#[test]
fn test_completion_private_member_reference_inside_packaged_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/PrivateMemberCompletion.kt").unwrap();
    let source = r#"
package com.example.service

class AccountService {
    private fun internalSecretOperation(): String = "secret"

    fun execute() {
        val ref = AccountService::intern
    }
}
"#;
    index.index_source(url.clone(), source);

    // Cursor at `AccountService::intern` (line 7, col 40)
    let pos = Position::new(7, 40);
    let completions = find_completions(&index, &url, pos, source);

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"internalSecretOperation"),
        "Private member reference `internalSecretOperation` should be completed within its declaring class in a package"
    );
}
