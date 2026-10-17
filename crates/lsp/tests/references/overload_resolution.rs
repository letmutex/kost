use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_overload_resolution_does_not_disqualify_target_when_exact_arity_overload_has_incompatible_argument_type()
 {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Logger.kt").unwrap();
    let source = r#"
package com.example

class Logger {
    fun log(message: String, level: Int = 1) {}
    fun log(code: Int) {}
}

fun runner(logger: Logger) {
    logger.log("hello")
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun log(message: String, level: Int = 1)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "log", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "logger.log(\"hello\") must match log(message: String, level: Int = 1)"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_resolves_trailing_lambda_call_when_preceding_parameter_has_default_value() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TaskRunner.kt").unwrap();
    let source = r#"
package com.example

class TaskRunner {
    fun runTask(timeout: Int = 1000, action: (String) -> Unit) {}
    fun runTask(timeout: Int = 1000, action: (Int) -> Unit) {}
}

fun runner(tr: TaskRunner) {
    tr.runTask { msg: String ->
        println(msg)
    }
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun runTask(timeout: Int = 1000, action: (String) -> Unit)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "runTask", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "Trailing lambda call tr.runTask {{ msg: String -> ... }} must match (String) -> Unit overload"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_excludes_function_call_when_target_is_non_functional_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Service.kt").unwrap();
    let source = r#"
package com.example

class Service {
    val status: String = "OK"
    fun status(code: Int) {}
}

fun runner(s: Service) {
    println(s.status)
    s.status(200)
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `val status: String` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "status", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "References to property val status: String must only match property access s.status, not method invocation s.status(200)"
    );
    assert_eq!(refs[0].range.start.line, 9);
}

#[test]
fn test_references_excludes_implicit_receiver_call_in_nested_class_with_same_simple_name_in_different_package()
 {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.example.outer

class Outer {
    class Inner {
        fun helper(): Unit {}
    }
}
"#;
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.other.service

class Container {
    class Inner {
        fun test() {
            helper()
        }
    }
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on `fun helper(): Unit {}` in Outer.Inner (line 5, col 12)
    let decl_pos = Position::new(5, 12);
    let refs = find_references(&index, &file_a_url, "helper", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        0,
        "helper() in unrelated Container.Inner must not match Outer.Inner.helper"
    );
}

#[test]
fn test_references_overload_resolution_disambiguates_null_literal_between_nullable_and_non_nullable_parameter()
 {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Handler.kt").unwrap();
    let source = r#"
package com.example

class Handler {
    fun handle(data: String) {}
    fun handle(data: String?) {}
}

fun runner(h: Handler) {
    h.handle(null)
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun handle(data: String)` (non-nullable overload, line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "handle", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        0,
        "h.handle(null) must not match non-nullable handle(data: String)"
    );
}

#[test]
fn test_references_includes_function_call_when_target_is_functional_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CallbackService.kt").unwrap();
    let source = r#"
package com.example

class CallbackService {
    val callback: () -> Unit = {}
}

fun runner(s: CallbackService) {
    s.callback()
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `val callback: () -> Unit` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "callback", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        1,
        "s.callback() must match functional property val callback: () -> Unit"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_excludes_implicit_receiver_call_in_same_named_class_in_different_package() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.example

class Service {
    fun execute(): Unit {}
}
"#;
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.other

class Service {
    fun runner() {
        execute()
    }
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on `fun execute(): Unit {}` in com.example.Service (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_a_url, "execute", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        0,
        "Implicit call execute() in com.other.Service must not match com.example.Service.execute"
    );
}

#[test]
fn test_references_overload_resolution_disambiguates_lambda_parameter_types_with_unit_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Processor.kt").unwrap();
    let source = r#"
package com.example

class Processor {
    fun process(handler: (String) -> Unit) {}
    fun process(handler: (Int) -> Unit) {}
}

fun runner(p: Processor) {
    p.process { s: String -> println(s) }
}
"#;
    index.index_source(url.clone(), source);

    // Cursor on `fun process(handler: (Int) -> Unit)` (line 5, col 8)
    let decl_pos = Position::new(5, 8);
    let refs = find_references(&index, &url, "process", decl_pos, source, false);

    assert_eq!(
        refs.len(),
        0,
        "Call with (String) -> Unit lambda must not match (Int) -> Unit overload despite Unit return"
    );
}

#[test]
fn test_references_resolves_inherited_member_when_subclass_file_has_star_import_and_super_in_sibling_file()
 {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.example

open class BaseService {
    fun serve(): Unit {}
}
"#;
    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.example

import java.util.*

class AppService : BaseService()

fun runner(app: AppService) {
    app.serve()
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Cursor on `fun serve(): Unit {}` in BaseService (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_a_url, "serve", decl_pos, file_a_code, false);

    assert_eq!(
        refs.len(),
        1,
        "app.serve() must match inherited serve() even when subclass file has star import"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_overload_resolution_disambiguates_lambda_parameter_nullability() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NullabilityOverload.kt").unwrap();
    let code = r#"
package com.example

class Handler {
    fun process(cb: (String) -> Unit) {}
    fun process(cb: (String?) -> Unit) {}
}

fun runner(h: Handler) {
    val f: (String?) -> Unit = {}
    h.process(f)
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun process(cb: (String) -> Unit)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "process", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "h.process(f) with (String?) -> Unit must not match fun process(cb: (String) -> Unit)"
    );
}
