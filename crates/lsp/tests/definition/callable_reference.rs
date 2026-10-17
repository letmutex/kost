use lsp::features::find_definition;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_definition_class_literal_receiver_resolves_to_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ClassLiteralDef.kt").unwrap();
    let source = r#"
package com.example

class TargetModel(val id: Int)

fun main() {
    val kClass = TargetModel::class
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `TargetModel` in `TargetModel::class` (line 6, col 20)
    let pos = Position::new(6, 20);
    let loc = find_definition(&index, &url, "TargetModel", pos, source)
        .expect("TargetModel class should resolve from class literal");

    assert_eq!(loc.uri, url);
    assert_eq!(loc.range.start.line, 3);
}

#[test]
fn test_definition_callable_reference_prioritizes_function_over_local_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LocalShadowingDef.kt").unwrap();
    let source = r#"
package com.example

fun performOperation(): Int = 42

fun main() {
    val performOperation = 100
    val fnRef = ::performOperation
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `performOperation` in `::performOperation` (line 7, col 22)
    let pos = Position::new(7, 22);
    let loc = find_definition(&index, &url, "performOperation", pos, source)
        .expect("::performOperation should resolve to top-level function, not local variable");

    assert_eq!(loc.uri, url);
    assert_eq!(
        loc.range.start.line, 3,
        "Should jump to top-level function on line 3, not local variable on line 6"
    );
}

#[test]
fn test_definition_callable_reference_resolves_outer_local_function_shadowed_by_inner_variable() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedLocalShadowingDef.kt").unwrap();
    let source = r#"
package com.example

fun main() {
    fun performAction(): Int = 42
    run {
        val performAction = 100
        val fnRef = ::performAction
    }
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `performAction` in `val fnRef = ::performAction` (line 7, col 23)
    let pos = Position::new(7, 23);
    let loc = find_definition(&index, &url, "performAction", pos, source)
        .expect("::performAction should resolve to outer local function on line 4");

    assert_eq!(loc.uri, url);
    assert_eq!(
        loc.range.start.line, 4,
        "Should jump to outer local function on line 4, not be blocked by inner local variable on line 6"
    );
}

#[test]
fn test_definition_member_callable_reference_resolves_to_member_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberDef.kt").unwrap();
    let source = r#"
package com.example

class Worker {
    fun doWork(): String = "done"
}

fun main() {
    val ref = Worker::doWork
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `doWork` in `Worker::doWork` (line 8, col 23)
    let pos = Position::new(8, 23);
    let loc = find_definition(&index, &url, "doWork", pos, source)
        .expect("Worker::doWork should resolve to member function");

    assert_eq!(loc.uri, url);
    assert_eq!(loc.range.start.line, 4);
}

#[test]
fn test_definition_constructor_callable_reference_resolves_to_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ConstructorDef.kt").unwrap();
    let source = r#"
package com.example

class Engine(val power: Int)

fun main() {
    val factory = ::Engine
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `Engine` in `::Engine` (line 6, col 21)
    let pos = Position::new(6, 21);
    let loc = find_definition(&index, &url, "Engine", pos, source)
        .expect("::Engine should resolve to class Engine");

    assert_eq!(loc.uri, url);
    assert_eq!(loc.range.start.line, 3);
}

#[test]
fn test_definition_extension_callable_reference_resolves_to_extension_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionDef.kt").unwrap();
    let source = r#"
package com.example

class Payload

fun Payload.process(): Boolean = true

fun main() {
    val ref = Payload::process
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `process` in `Payload::process` (line 8, col 24)
    let pos = Position::new(8, 24);
    let loc = find_definition(&index, &url, "process", pos, source)
        .expect("Payload::process should resolve to extension function");

    assert_eq!(loc.uri, url);
    assert_eq!(loc.range.start.line, 5);
}
