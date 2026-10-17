use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_inferred_variable_receiver_resolves_member_reference() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/InferredReceiver.kt").unwrap();
    let code = r#"
package com.example

interface Job {
    val isActive: Boolean
}

interface CoroutineContext {
    operator fun <E> get(key: Any): E?
}

fun test(context: CoroutineContext, condition: Boolean) {
    val job = if (condition) context[Job] else null
    if (job != null && !job.isActive) {
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `val isActive: Boolean` in Job (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "isActive", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "job.isActive on inferred variable job should match Job.isActive"
    );
    assert_eq!(refs[0].range.start.line, 13);
}

#[test]
fn test_literal_receiver_does_not_match_unrelated_class_member_reference() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Connection.kt").unwrap();
    let code = r#"
package com.example

class Connection {
    fun close() {}
}

fun test(conn: Connection) {
    conn.close()
    "hello".close()
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun close()` in Connection (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "close", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Only conn.close() should match Connection.close, string literal 'hello'.close() must not match"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_implicit_call_in_outer_class_does_not_match_nested_class_member() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NestedMember.kt").unwrap();
    let code = r#"
package com.example

class Outer {
    class Inner {
        fun helper() {}
    }

    fun run() {
        helper()
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun helper()` in Inner (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_url, "helper", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "Implicit call in Outer class should not match nested Inner class member"
    );
}

#[test]
fn test_implicit_call_in_nested_class_resolves_nested_class_member_reference() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NestedClass.kt").unwrap();
    let code = r#"
package com.example

class Outer {
    class Inner {
        fun action() {}
        fun call() {
            action()
        }
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun action()` in Inner (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_url, "action", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Implicit action() inside Outer.Inner.call should match Outer.Inner.action"
    );
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_implicit_call_in_nested_subclass_resolves_inherited_member_reference() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/NestedSubclass.kt").unwrap();
    let code = r#"
package com.example

open class Base {
    fun baseMethod() {}
}

class Outer {
    class Inner : Base() {
        fun run() {
            baseMethod()
        }
    }
}
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at `fun baseMethod()` in Base (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "baseMethod", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Implicit baseMethod() inside Outer.Inner.run should match Base.baseMethod"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_conforms_to_companion_rejects_container_with_unrelated_prefix_substring() {
    let mut index = WorkspaceIndex::new();
    let url_other = Url::parse("file:///project/OtherService.kt").unwrap();
    let code_other = r#"
package com.example

class OtherService {
    companion object Factory {
        fun create(): Unit {}
    }
}
"#;
    index.index_source(url_other.clone(), code_other);

    let conforms = lsp::features::references::conforms_to_container_or_companion(
        &index,
        "com.example.Service",
        "com.example.Service.Factory",
    );
    assert!(
        !conforms,
        "Service must not conform to Service.Factory via OtherService.Factory"
    );
}

#[test]
fn test_references_overload_disambiguates_lambda_returning_custom_type_ending_in_unit() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Dispatcher.kt").unwrap();
    let source = r#"
package com.example

class TimeUnit
class Result

class Dispatcher {
    fun schedule(task: () -> TimeUnit): Int = 1
    fun schedule(task: () -> Result): Int = 2
}

fun runner(d: Dispatcher) {
    d.schedule { Result() }
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun schedule(task: () -> TimeUnit)`: line 7, col 8
    let decl_pos = Position::new(7, 8);
    let refs = find_references(&index, &url, "schedule", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        0,
        "d.schedule with Result lambda should match schedule(() -> Result), not schedule(() -> TimeUnit)"
    );
}

#[test]
fn test_references_excludes_instance_member_call_colliding_with_single_segment_package_name() {
    let mut index = WorkspaceIndex::new();
    let log_url = Url::parse("file:///project/Log.kt").unwrap();
    let log_code = "package log\n\nfun info(msg: String) {}\n";
    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package app\n\nclass Logger {\n    fun info(msg: String) {}\n}\n\nfun runner(log: Logger) {\n    log.info(\"hello\")\n}\n";
    index.index_source(log_url.clone(), log_code);
    index.index_source(app_url.clone(), app_code);

    // Cursor on `fun info(msg: String)` in Log.kt: line 2, col 4
    let decl_pos = Position::new(2, 4);
    let refs = find_references(&index, &log_url, "info", decl_pos, log_code, false);

    assert_eq!(
        refs.len(),
        0,
        "log.info() on instance variable log should not match top-level fun info in package log"
    );
}

#[test]
fn test_conforms_to_companion_rejects_object_in_unrelated_package_same_class_name() {
    let mut index = WorkspaceIndex::new();
    let beta_url = Url::parse("file:///project/Beta.kt").unwrap();
    let beta_code = "package com.beta\n\nclass Container {\n    companion object Factory {\n        fun create(): Unit {}\n    }\n}\n";
    let alpha_url = Url::parse("file:///project/Alpha.kt").unwrap();
    let alpha_code = "package com.alpha\n\nclass Container\n";
    index.index_source(beta_url.clone(), beta_code);
    index.index_source(alpha_url.clone(), alpha_code);

    let conforms = lsp::features::references::conforms_to_container_or_companion(
        &index,
        "com.alpha.Container",
        "com.alpha.Container.Factory",
    );
    assert!(
        !conforms,
        "com.alpha.Container must not match Factory companion defined only in com.beta.Container"
    );
}

#[test]
fn test_references_resolves_qualified_callable_reference_on_nested_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedRef.kt").unwrap();
    let code = r#"
package com.example

class Outer {
    class Inner {
        fun action(): Unit {}
    }
}

val ref = Outer.Inner::action
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun action(): Unit {}` (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &url, "action", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Outer.Inner::action must be recognized as reference to Outer.Inner.action"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_excludes_outer_class_instance_call_for_normal_nested_object_member() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedObject.kt").unwrap();
    let code = r#"
package com.example

class Service {
    object Helper {
        fun run(): Unit {}
    }
}

fun test(s: Service) {
    s.run()
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun run(): Unit {}` in Service.Helper (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &url, "run", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "s.run() on outer class Service instance must not match member in normal nested object Helper"
    );
}
