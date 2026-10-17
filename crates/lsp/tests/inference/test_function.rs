use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_function_return_type, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_function_explicit_return_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FnExplicit.kt").unwrap();
    let code = r#"
package com.example.fn

class User(val name: String)

fun getUser(): User {
    return User("Alice")
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    assert_eq!(
        infer_function_return_type(&index, &url, "getUser", pos, code, None),
        KotlinType::parse("com.example.fn.User")
    );
}

#[test]
fn test_infer_function_single_expression_body() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FnExpr.kt").unwrap();
    let code = r#"
package com.example.fn

class Session(val token: String)

fun createSession() = Session("abc-123")
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    assert_eq!(
        infer_function_return_type(&index, &url, "createSession", pos, code, None),
        KotlinType::parse("com.example.fn.Session")
    );
}

#[test]
fn test_infer_function_constructor_identifier() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Constructor.kt").unwrap();
    let code = r#"
package com.example.ctor

class Widget(val width: Int)
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(3, 4);
    assert_eq!(
        infer_function_return_type(&index, &url, "Widget", pos, code, None),
        KotlinType::parse("com.example.ctor.Widget")
    );
}

#[test]
fn test_infer_function_single_expression_extension_this() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtFn.kt").unwrap();
    let code = r#"
package com.example.extfn

interface Modifier {
    fun then(other: Modifier): Modifier
}

fun Modifier.custom(x: Int) = this.then(this)
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    assert_eq!(
        infer_function_return_type(&index, &url, "custom", pos, code, None),
        KotlinType::parse("com.example.extfn.Modifier")
    );
}

#[test]
fn test_infer_function_single_expression_literal() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LiteralFn.kt").unwrap();
    let code = r#"
package com.example.literal

fun getCount() = 42
fun getFlag() = true
fun getGreeting() = "hello"
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(3, 4);
    assert_eq!(
        infer_function_return_type(&index, &url, "getCount", pos, code, None),
        KotlinType::parse("Int")
    );
    assert_eq!(
        infer_function_return_type(&index, &url, "getFlag", pos, code, None),
        KotlinType::parse("Boolean")
    );
    assert_eq!(
        infer_function_return_type(&index, &url, "getGreeting", pos, code, None),
        KotlinType::parse("String")
    );
}

#[test]
fn test_infer_function_single_expression_method_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MethodFn.kt").unwrap();
    let code = r#"
package com.example.method

class Profile(val bio: String)
class Service {
    fun fetchProfile(): Profile = Profile("bio")
}

val service = Service()
fun getProfile() = service.fetchProfile()
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    assert_eq!(
        infer_function_return_type(&index, &url, "getProfile", pos, code, None),
        KotlinType::parse("com.example.method.Profile")
    );
}

#[test]
fn test_infer_pascal_case_function_return_type_not_shadowed_by_constructor_heuristic() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Compose.kt").unwrap();
    let code = r#"
package com.example.ui

fun AppTheme(): String {
    return "dark"
}

val currentTheme = AppTheme()
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "currentTheme", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_multiple_positional_lambdas_map_to_respective_parameter_types() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TwoLambdas.kt").unwrap();
    let code = r#"
package com.example.fn

class ScopeA { fun a() {} }
class ScopeB { fun b() {} }

fun runTwo(first: ScopeA.() -> Unit, second: ScopeB.() -> Unit) {}

fun test() {
    runTwo({
        val x = 1
    }, {
        val y = 2
    })
}
"#;
    index.index_source(url.clone(), code);

    // Cursor inside the first lambda (line 10, col 12)
    let pos_a = Position::new(10, 12);
    let scope_a = lsp::inference::infer_scope_receiver(&index, &url, "this", pos_a, code, None);
    assert_eq!(
        scope_a,
        KotlinType::parse("com.example.fn.ScopeA"),
        "First positional lambda should infer ScopeA as receiver, not ScopeB"
    );

    // Cursor inside the second lambda (line 12, col 12)
    let pos_b = Position::new(12, 12);
    let scope_b = lsp::inference::infer_scope_receiver(&index, &url, "this", pos_b, code, None);
    assert_eq!(
        scope_b,
        KotlinType::parse("com.example.fn.ScopeB"),
        "Second positional lambda should infer ScopeB as receiver, not ScopeA"
    );
}

#[test]
fn test_infer_function_call_named_do_preserves_custom_return_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DoCall.kt").unwrap();
    let code = r#"
package com.example.fn

fun do(count: Int): String = "done"

fun run() {
    val result = do(5)
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "result", pos, code, None),
        Some(KotlinType::STRING)
    );
}
