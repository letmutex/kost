use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_hover_implicit_it_negative_when_explicit_params() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NegativeTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun test(users: List<User>) {
    users.forEach { u ->
        val x = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(11, 16);
    let hover = find_hover(&index, &url, "it", pos, source);
    assert!(
        hover.is_none(),
        "Hover on it should be None when lambda has explicit parameter 'u ->'"
    );
}

#[test]
fn test_hover_implicit_it_negative_on_member_selector() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberSelectorTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun test(users: List<User>) {
    users.forEach {
        val x = unknownObj.it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(11, 27); // on `it` in `unknownObj.it`
    let hover = find_hover(&index, &url, "it", pos, source);
    assert!(
        hover.is_none(),
        "Hover on member selector 'unknownObj.it' should not resolve to outer lambda 'it'"
    );
}

#[test]
fn test_hover_implicit_it_negative_inside_local_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalFunTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun test(users: List<User>) {
    users.forEach {
        fun localHelper() {
            val x = it
        }
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(12, 20); // on `it` inside `localHelper()`
    let hover = find_hover(&index, &url, "it", pos, source);
    assert!(
        hover.is_none(),
        "Hover on it inside local function should be None (not inherit lambda's it)"
    );
}
