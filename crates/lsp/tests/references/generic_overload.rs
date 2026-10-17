use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_resolves_top_level_generic_function_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericCall.kt").unwrap();
    let code = r#"
package com.example

fun <T> identity(item: T): T = item
fun identity(code: Int, extra: String): Int = code

fun runner() {
    val x = identity(42)
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun <T> identity(item: T)` (line 3, col 8)
    let decl_pos = Position::new(3, 8);
    let refs = find_references(&index, &url, "identity", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "identity(42) should be resolved as a reference to fun <T> identity(item: T)"
    );
    assert_eq!(refs[0].range.start.line, 7);
}

#[test]
fn test_references_overload_resolution_disambiguates_lambda_return_type_nullability() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ReturnNullability.kt").unwrap();
    let code = r#"
package com.example

class TaskRunner {
    fun run(action: () -> String) {}
    fun run(action: () -> String?) {}
}

fun runner(tr: TaskRunner) {
    val f: () -> String? = { null }
    tr.run(f)
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun run(action: () -> String)` (non-nullable return, line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "run", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "tr.run(f) with lambda returning String? must not match fun run(action: () -> String)"
    );
}

#[test]
fn test_references_alternative_overload_better_match_requires_compatible_arity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ArityMatch.kt").unwrap();
    let code = r#"
package com.example

class Service {
    fun execute(s: String) {}
    fun execute(code: Any, retries: Int, verbose: Boolean) {}
}

fun runner(s: Service) {
    val x = "test"
    s.execute(x, 3, true)
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun execute(code: Any, retries: Int, verbose: Boolean)` (line 5, col 8)
    let decl_pos = Position::new(5, 8);
    let refs = find_references(&index, &url, "execute", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "s.execute(x, 3, true) must match 3-param overload and not be disqualified by 1-param overload"
    );
    assert_eq!(refs[0].range.start.line, 10);
}

#[test]
fn test_references_overload_resolution_rejects_zero_arg_lambda_for_binary_functional_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LambdaArity.kt").unwrap();
    let code = r#"
package com.example

class Handler {
    fun handle(callback: (String, Int) -> Unit) {}
    fun handle(action: () -> Unit) {}
}

fun runner(h: Handler) {
    h.handle { println() }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun handle(callback: (String, Int) -> Unit)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "handle", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "h.handle {{ println() }} must not resolve to handle(callback: (String, Int) -> Unit)"
    );
}

#[test]
fn test_references_excludes_call_with_extra_trailing_lambda_when_no_lambda_parameter_available() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TrailingVararg.kt").unwrap();
    let code = r#"
package com.example

class Logger {
    fun log(tag: String, vararg messages: String) {}
    fun log(tag: String, action: () -> Unit) {}
}

fun runner(l: Logger) {
    l.log("TAG", "msg") { println() }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun log(tag: String, vararg messages: String)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "log", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "Call with trailing lambda must not match overload where all parameters are consumed by value arguments"
    );
}

#[test]
fn test_references_finds_non_call_usage_of_functional_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/PropertyRef.kt").unwrap();
    let code = r#"
package com.example

class Button {
    val onClick: (count: Int) -> Unit = {}
}

fun runner(b: Button) {
    val handler = b.onClick
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `val onClick: (count: Int) -> Unit = {}` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "onClick", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Property access `b.onClick` should be found as reference to `val onClick: (count: Int) -> Unit`"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_resolves_implicit_receiver_call_for_subsequent_member_overload() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ImplicitOverload.kt").unwrap();
    let code = r#"
package com.example

class Calculator {
    fun calculate(num: Int): Int = num
    fun calculate(str: String): String = str

    fun test() {
        val res = calculate("hello")
    }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun calculate(str: String): String = str` (line 5, col 8)
    let decl_pos = Position::new(5, 8);
    let refs = find_references(&index, &url, "calculate", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        1,
        "Unqualified call calculate(\"hello\") must resolve to second overload calculate(str: String)"
    );
    assert_eq!(refs[0].range.start.line, 8);
}

#[test]
fn test_references_excludes_call_leaving_required_parameter_unbound() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UnboundParam.kt").unwrap();
    let code = r#"
package com.example

class Service {
    fun process(id: Int, name: String, timeout: Double = 1.0) {}
}

fun runner(s: Service) {
    s.process(id = 1, timeout = 5.0)
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun process(id: Int, name: String, timeout: Double = 1.0)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "process", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "Call s.process(id = 1, timeout = 5.0) must not match overload with missing required parameter 'name'"
    );
}

#[test]
fn test_references_overload_disambiguation_for_trailing_lambda_with_different_arity_alternatives() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TrailingArity.kt").unwrap();
    let code = r#"
package com.example

class Dispatcher {
    fun submit(tag: String = "default", action: (Int) -> String) {}
    fun submit(tag: String = "default", retries: Int = 0, action: () -> Unit) {}
}

fun runner(d: Dispatcher) {
    d.submit { println("task") }
}
"#;
    index.index_source(url.clone(), code);

    // Cursor on `fun submit(tag: String = "default", action: (Int) -> String)` (line 4, col 8)
    let decl_pos = Position::new(4, 8);
    let refs = find_references(&index, &url, "submit", decl_pos, code, false);

    assert_eq!(
        refs.len(),
        0,
        "Trailing lambda () -> Unit must not match overload taking (Int) -> String when alternative overload matches"
    );
}
