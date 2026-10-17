use lsp::features::definition::find_definition;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_cross_file_overload_in_same_package_not_blocked_by_current_file_incompatible_overload() {
    let mut index = WorkspaceIndex::new();
    let url_a = Url::parse("file:///project/SenderA.kt").unwrap();
    let code_a = r#"
package com.example

fun send(code: Int): Unit {}

fun test() {
    send("hello")
}
"#;
    let url_b = Url::parse("file:///project/SenderB.kt").unwrap();
    let code_b = r#"
package com.example

fun send(msg: String): Unit {}
"#;

    index.index_source(url_a.clone(), code_a);
    index.index_source(url_b.clone(), code_b);

    // Position on `send` in `send("hello")` in SenderA.kt at line 6, col 4
    let pos = Position::new(6, 4);
    let def = find_definition(&index, &url_a, "send", pos, code_a)
        .expect("send(\"hello\") should resolve to a definition");

    assert_eq!(
        def.uri, url_b,
        "send(\"hello\") should resolve to send(msg: String) in SenderB.kt, but was incorrectly blocked by incompatible send(code: Int) in SenderA.kt"
    );
}

#[test]
fn test_trailing_lambda_call_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun launch(action: () -> Unit) {}

fun test() {
    launch {
        val value = 1 + 2
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 4);
    let def = find_definition(&index, &url, "launch", pos, code)
        .expect("launch { ... } should resolve to definition");

    assert_eq!(def.uri, url);
    assert_eq!(def.range.start.line, 3);
}

#[test]
fn test_overloaded_call_with_trailing_lambda_selects_matching_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun execute(name: String, action: () -> Unit) {}
fun execute(count: Int, action: () -> Unit) {}

fun test() {
    execute("task") {
        val value = 42
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let def = find_definition(&index, &url, "execute", pos, code)
        .expect("execute(\"task\") { ... } should resolve to String overload");

    assert_eq!(def.uri, url);
    assert_eq!(def.range.start.line, 3);
}

#[test]
fn test_overloaded_call_with_lambda_return_type_selects_matching_signature() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun transform(action: () -> Int): Int = action()
fun transform(action: () -> String): String = action()

fun test() {
    transform { "hello" }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let def = find_definition(&index, &url, "transform", pos, code)
        .expect("transform { \"hello\" } should resolve to String overload");

    assert_eq!(def.uri, url);
    assert_eq!(def.range.start.line, 4);
}

#[test]
fn test_qualified_overloaded_call_selects_matching_signature() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

class Service {
    fun handle(count: Int) {}
    fun handle(data: String) {}
}

fun test(s: Service) {
    s.handle("test")
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 6);
    let def = find_definition(&index, &url, "handle", pos, code)
        .expect("s.handle(\"test\") should resolve to String overload");

    assert_eq!(def.uri, url);
    assert_eq!(def.range.start.line, 5);
}

#[test]
fn test_annotated_lambda_call_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Runner.kt").unwrap();
    let code = r#"
package com.example

fun execute(action: () -> Unit) {}

fun test() {
    execute(@Composable {
        val count = 1
    })
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 4);
    let def = find_definition(&index, &url, "execute", pos, code)
        .expect("execute(@Composable { ... }) should resolve to definition");

    assert_eq!(def.uri, url);
    assert_eq!(def.range.start.line, 3);
}

#[test]
fn test_member_call_with_different_arity_bypasses_incompatible_subclass_decl_and_resolves_superclass()
 {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AritySubclass.kt").unwrap();
    let code = r#"
package com.example

open class Parent {
    fun execute(): Unit {}
}

class Child : Parent() {
    fun execute(code: Int): Unit {}
}

fun test(child: Child) {
    child.execute()
}
"#;
    index.index_source(url.clone(), code);

    // Position on `execute` in `child.execute()` (line 12, col 10)
    let pos = Position::new(12, 10);
    let def = find_definition(&index, &url, "execute", pos, code)
        .expect("child.execute() should resolve to Parent.execute");

    assert_eq!(
        def.range.start.line, 4,
        "Zero-arg child.execute() should resolve to Parent.execute() at line 4, not Child.execute(code: Int) at line 8"
    );
}

#[test]
fn test_implicit_this_call_to_overloaded_member_resolves_matching_signature() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ImplicitThisOverload.kt").unwrap();
    let code = r#"
package com.example

class Service {
    fun process(): Unit {}
    fun process(x: Int): Unit {}

    fun run() {
        process(10)
    }
}
"#;
    index.index_source(url.clone(), code);

    // Position on `process` in `process(10)` (line 8, col 8)
    let pos = Position::new(8, 8);
    let def = find_definition(&index, &url, "process", pos, code)
        .expect("Implicit this call process(10) should resolve definition");

    assert_eq!(
        def.range.start.line, 5,
        "process(10) should resolve to fun process(x: Int) at line 5, not fun process() at line 4"
    );
}

#[test]
fn test_definition_direct_call_resolves_to_functional_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FunctionalParamCall.kt").unwrap();
    let code = r#"
package com.example

fun execute(callback: () -> Unit) {
    callback()
}
"#;
    index.index_source(url.clone(), code);

    // Position on `callback` in `callback()` (line 4, col 4)
    let pos = Position::new(4, 4);
    let def = find_definition(&index, &url, "callback", pos, code)
        .expect("callback() should resolve to functional parameter callback");

    assert_eq!(
        def.range.start.line, 3,
        "callback() should resolve to parameter callback on line 3"
    );
}
