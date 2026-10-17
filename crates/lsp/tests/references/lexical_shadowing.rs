use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_excludes_implicit_member_access_shadowed_by_local_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Worker.kt").unwrap();
    let code = r#"
package com.example

class Worker {
    val status: String = "active"

    fun checkStatus() {
        val status = 42
        println(status)
    }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at property `val status: String` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "status", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "println(status) must not match property Worker.status when shadowed by local variable"
    );
}

#[test]
fn test_references_excludes_implicit_member_access_shadowed_by_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Worker.kt").unwrap();
    let code = r#"
package com.example

class Worker {
    val status: String = "active"

    fun update(status: String) {
        println(status)
    }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at property `val status: String` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "status", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "println(status) must not match property Worker.status when shadowed by parameter"
    );
}

#[test]
fn test_references_excludes_top_level_function_shadowed_by_inner_parameter_direct_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Task.kt").unwrap();
    let code = r#"
package com.example

fun execute() {}

class Task {
    fun run(execute: () -> Unit) {
        execute()
    }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at top-level `fun execute()` (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &url, "execute", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "execute() inside run must not match top-level execute() when shadowed by parameter"
    );
}

#[test]
fn test_references_excludes_top_level_function_shadowed_by_inner_local_variable_direct_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun notify() {}

fun runner() {
    val notify = { println("local") }
    notify()
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at top-level `fun notify()` (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &url, "notify", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "notify() inside runner must not match top-level notify() when shadowed by local variable"
    );
}
