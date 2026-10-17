use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_generic_type_annotated_variable_member_completion_and_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericBoxTest.kt").unwrap();
    let code = r#"
package com.example.generics

class Box<T>(val item: T) {
    fun getItemValue(): T = item
}

fun test(box: Box<String>) {
    box.
    val i = box.item
    val v = box.getItemValue()
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion on `box.` at line 8, col 8:
    let pos_dot = Position::new(8, 8);
    let completions = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"item".to_string()),
        "box. on Box<String> should offer 'item'"
    );
    assert!(
        labels.contains(&"getItemValue".to_string()),
        "box. on Box<String> should offer 'getItemValue'"
    );

    // 2. Definition on `item` at line 9, col 17:
    let pos_item = Position::new(9, 17);
    let def_item = find_definition(&index, &url, "item", pos_item, code);
    assert!(
        def_item.is_some(),
        "box.item on Box<String> should resolve definition of 'item'"
    );

    // 3. Hover on `getItemValue` at line 10, col 17:
    let pos_val = Position::new(10, 17);
    let hover_val = find_hover(&index, &url, "getItemValue", pos_val, code);
    assert!(
        hover_val.is_some(),
        "box.getItemValue() on Box<String> should resolve hover on 'getItemValue'"
    );
}

#[test]
fn test_generic_extension_function_completion_and_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericExtensionTest.kt").unwrap();
    let code = r#"
package com.example.ext

class Container<T>(val value: T)

fun <T> Container<T>.unwrap(): T = value

fun test(c: Container<String>) {
    c.
    val u = c.unwrap()
}
"#;
    index.index_source(url.clone(), code);

    // 1. Completion on `c.` at line 8, col 6:
    let pos_dot = Position::new(8, 6);
    let completions = find_completions(&index, &url, pos_dot, code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"unwrap".to_string()),
        "c. on Container<String> should offer generic extension 'unwrap'"
    );

    // 2. Definition on `unwrap` at line 9, col 15:
    let pos_unwrap = Position::new(9, 15);
    let def_unwrap = find_definition(&index, &url, "unwrap", pos_unwrap, code);
    assert!(
        def_unwrap.is_some(),
        "c.unwrap() should resolve definition of generic extension"
    );

    // 3. Hover on `unwrap`:
    let hover_unwrap = find_hover(&index, &url, "unwrap", pos_unwrap, code);
    assert!(
        hover_unwrap.is_some(),
        "c.unwrap() should resolve hover on generic extension"
    );
}
