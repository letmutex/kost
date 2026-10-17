use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

fn find_position_of(source: &str, pattern: &str) -> Position {
    let offset = source
        .find(pattern)
        .unwrap_or_else(|| panic!("pattern '{}' not found in source", pattern));
    let mut line = 0;
    let mut character = 0;
    for (i, ch) in source.char_indices() {
        if i == offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Position { line, character }
}

#[test]
fn test_multi_line_function_call_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Call.kt").unwrap();
    let source = r#"package com.example

fun someFunction(name: String, age: Int, isHuman: Boolean) {}

fun caller() {
    someFunction(
        
    )
}
"#;
    index.index_source(url.clone(), source);

    let pos = find_position_of(source, "\n    )");
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"name".to_string()),
        "Completion inside multi-line someFunction( should offer 'name', actual: {:?}",
        labels
    );
    assert!(
        labels.contains(&"age".to_string()),
        "Completion inside multi-line someFunction( should offer 'age', actual: {:?}",
        labels
    );
    assert!(
        labels.contains(&"isHuman".to_string()),
        "Completion inside multi-line someFunction( should offer 'isHuman', actual: {:?}",
        labels
    );
}

#[test]
fn test_multi_line_function_call_with_existing_argument_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CallArgs.kt").unwrap();
    let source = r#"package com.example

fun someFunction(name: String, age: Int, isHuman: Boolean) {}

fun caller() {
    someFunction(
        name = "Alice",
        
    )
}
"#;
    index.index_source(url.clone(), source);

    let pos = find_position_of(source, "\n    )");
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"age".to_string()),
        "Completion inside multi-line someFunction( with name passed should offer 'age', actual: {:?}",
        labels
    );
    assert!(
        labels.contains(&"isHuman".to_string()),
        "Completion inside multi-line someFunction( with name passed should offer 'isHuman', actual: {:?}",
        labels
    );
}

#[test]
fn test_multi_line_member_method_call_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberCall.kt").unwrap();
    let source = r#"package com.example

class UserService {
    fun someFunction(name: String, age: Int, isHuman: Boolean) {}
}

fun caller(service: UserService) {
    service.someFunction(
        
    )
}
"#;
    index.index_source(url.clone(), source);

    let pos = find_position_of(source, "\n    )");
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"name".to_string()),
        "Completion inside multi-line service.someFunction( should offer 'name', actual: {:?}",
        labels
    );
    assert!(
        labels.contains(&"age".to_string()),
        "Completion inside multi-line service.someFunction( should offer 'age', actual: {:?}",
        labels
    );
    assert!(
        labels.contains(&"isHuman".to_string()),
        "Completion inside multi-line service.someFunction( should offer 'isHuman', actual: {:?}",
        labels
    );
}
