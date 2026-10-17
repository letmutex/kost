use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_apply_scope_function_this_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ApplyTest.kt").unwrap();
    let code = r#"
package com.example.scope

class Window(var width: Int, var height: Int) {
    fun resize(w: Int, h: Int) {}
}

fun setup() {
    val win = Window(800, 600)
    win.apply {
        this.
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 13);
    let comps = find_completions(&index, &url, pos, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"width".to_string()),
        "this. inside apply should offer property 'width'"
    );
    assert!(
        labels.contains(&"height".to_string()),
        "this. inside apply should offer property 'height'"
    );
    assert!(
        labels.contains(&"resize".to_string()),
        "this. inside apply should offer method 'resize'"
    );
}

#[test]
fn test_with_scope_function_this_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/WithTest.kt").unwrap();
    let code = r#"
package com.example.scope

class Window(var width: Int, var height: Int) {
    fun resize(w: Int, h: Int) {}
}

fun setup() {
    val win = Window(800, 600)
    with(win) {
        this.
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 13);
    let comps = find_completions(&index, &url, pos, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"width".to_string()),
        "this. inside with(win) should offer property 'width'"
    );
    assert!(
        labels.contains(&"resize".to_string()),
        "this. inside with(win) should offer method 'resize'"
    );
}

#[test]
fn test_let_scope_function_it_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LetTest.kt").unwrap();
    let code = r#"
package com.example.scope

class Session(val token: String) {
    fun terminate(): Boolean = true
}

fun test(session: Session) {
    session.let {
        it.
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 11);
    let comps = find_completions(&index, &url, pos, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"token".to_string()),
        "it. inside session.let should offer property 'token'"
    );
    assert!(
        labels.contains(&"terminate".to_string()),
        "it. inside session.let should offer method 'terminate'"
    );
}

#[test]
fn test_also_scope_function_it_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AlsoTest.kt").unwrap();
    let code = r#"
package com.example.scope

class Session(val token: String) {
    fun terminate(): Boolean = true
}

fun test(session: Session) {
    session.also {
        it.
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 11);
    let comps = find_completions(&index, &url, pos, code);
    let labels: Vec<String> = comps.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"token".to_string()),
        "it. inside session.also should offer property 'token'"
    );
    assert!(
        labels.contains(&"terminate".to_string()),
        "it. inside session.also should offer method 'terminate'"
    );
}
