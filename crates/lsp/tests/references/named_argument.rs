use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_overload_resolution_rejects_unmatched_named_argument() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Renderer.kt").unwrap();
    let code = r#"
package com.example

class Renderer {
    fun render(width: Int) {}
    fun render(height: Int) {}
}

fun test(renderer: Renderer) {
    renderer.render(height = 100)
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at `fun render(width: Int)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "render", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "render(height = 100) must not match render(width: Int) when named argument does not match parameter name"
    );
}

#[test]
fn test_references_subclass_overload_with_different_argument_type_not_matched_to_super() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/HierarchyOverload.kt").unwrap();
    let code = r#"
package com.example

open class Parent {
    fun execute(code: Int) {}
}

class Child : Parent() {
    fun execute(name: String) {}
}

fun test(child: Child) {
    child.execute("start")
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at `Parent.execute(code: Int)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "execute", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "child.execute(\"start\") must not match Parent.execute(code: Int) when argument type is String"
    );
}

#[test]
fn test_references_trailing_lambda_call_with_multiple_default_parameters_resolves_reference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Builder.kt").unwrap();
    let code = r#"
package com.example

fun launch(context: String = "", flag: Boolean = false, block: () -> Unit) {}

fun test() {
    launch {
        println("started")
    }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor at `fun launch` (line 3, col 4)
    let decl_pos = Position::new(3, 4);
    let refs = find_references(&index, &url, "launch", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "launch call with only trailing lambda must match launch with default arguments"
    );
    assert_eq!(refs[0].range.start.line, 6);
}
