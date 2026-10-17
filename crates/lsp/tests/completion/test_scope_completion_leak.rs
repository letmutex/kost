use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_local_variables_do_not_leak_to_other_functions() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ScopeTest.kt").unwrap();
    let code = r#"
package com.example.scope

fun foo() {
    val localVarFoo = 123
}

fun bar() {
    val localVarBar = 456
    
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(9, 4), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"localVarBar".to_string()),
        "localVarBar should be visible inside bar()"
    );
    assert!(
        !labels.contains(&"localVarFoo".to_string()),
        "localVarFoo from foo() must NOT leak into bar()"
    );
}

#[test]
fn test_local_variables_do_not_leak_to_global_scope() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GlobalScopeTest.kt").unwrap();
    let code = r#"
package com.example.scope

fun calculateSomething() {
    val secretHelperVar = "hidden"
}

fun main() {
    
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(8, 4), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"calculateSomething".to_string()),
        "Top-level function calculateSomething should be visible"
    );
    assert!(
        labels.contains(&"main".to_string()),
        "Top-level function main should be visible"
    );
    assert!(
        !labels.contains(&"secretHelperVar".to_string()),
        "Local variable secretHelperVar must NOT leak into main()"
    );
}

#[test]
fn test_class_members_do_not_leak_to_global_scope_without_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UserTest.kt").unwrap();
    let code = r#"
package com.example.model

class User(val id: String, val name: String) {
    val isActive: Boolean = true
    fun getDisplayName(): String = name
}

fun outsideFunction() {
    
}
"#;
    index.index_source(url.clone(), code);

    let completions = find_completions(&index, &url, Position::new(10, 4), code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"User".to_string()),
        "Class User should be visible at top level"
    );
    assert!(
        labels.contains(&"outsideFunction".to_string()),
        "outsideFunction should be visible at top level"
    );
    assert!(
        !labels.contains(&"id".to_string()),
        "User.id property must NOT leak to top-level scope"
    );
    assert!(
        !labels.contains(&"name".to_string()),
        "User.name property must NOT leak to top-level scope"
    );
    assert!(
        !labels.contains(&"isActive".to_string()),
        "User.isActive property must NOT leak to top-level scope"
    );
    assert!(
        !labels.contains(&"getDisplayName".to_string()),
        "User.getDisplayName() method must NOT leak to top-level scope"
    );
}

#[test]
fn test_local_variables_do_not_appear_in_completion_before_declaration() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BackwardsLeakTest.kt").unwrap();
    let code = r#"
package com.example.scope

fun process() {
    
    val lateDeclaredVariable = 42
    val anotherLateVariable = "hello"
}
"#;
    index.index_source(url.clone(), code);

    // Position at line 4, col 4 (before `lateDeclaredVariable` is declared):
    let pos_before = Position::new(4, 4);
    let completions = find_completions(&index, &url, pos_before, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        !labels.contains(&"lateDeclaredVariable".to_string()),
        "Local variable 'lateDeclaredVariable' must NOT be offered in completions before its declaration"
    );
    assert!(
        !labels.contains(&"anotherLateVariable".to_string()),
        "Local variable 'anotherLateVariable' must NOT be offered in completions before its declaration"
    );
}
