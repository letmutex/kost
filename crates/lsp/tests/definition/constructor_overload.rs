use lsp::features::find_definition;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_supertype_constructor_invocation_overload_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ConstructorInvocation.kt").unwrap();
    let code = r#"
package com.example.test

open class Base {
    constructor(id: Int)
    constructor(id: Int, name: String)
}

class Sub : Base(10, "test")
"#;
    index.index_source(url.clone(), code);

    // Position on `Base` in `class Sub : Base(10, "test")` at line 8, col 13
    let pos = Position::new(8, 13);
    let def = find_definition(&index, &url, "Base", pos, code)
        .expect("Definition should resolve for supertype constructor invocation");

    // Line 5 is secondary constructor `constructor(id: Int, name: String)`
    assert_eq!(
        def.range.start.line, 5,
        "Base(10, \"test\") should resolve to the 2-parameter constructor on line 5"
    );
}
