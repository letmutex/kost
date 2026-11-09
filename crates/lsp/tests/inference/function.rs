use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_function_return_type, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn infer_function_explicit_return_type() {
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
fn infer_function_single_expression_body() {
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
fn infer_function_constructor_identifier() {
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
fn infer_function_single_expression_extension_this() {
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
fn infer_function_single_expression_literal() {
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
fn infer_function_single_expression_method_call() {
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
fn infer_pascal_case_function_return_type_not_shadowed_by_constructor_heuristic() {
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
fn multiple_positional_lambdas_map_to_respective_parameter_types() {
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
fn infer_function_call_named_do_preserves_custom_return_type() {
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

fn setup_compose_source_jar_index() -> WorkspaceIndex {
    let mut index = WorkspaceIndex::new();
    let modifier_jar_url =
        Url::parse("jar:file:///caches/ui-sources.jar!/androidx/compose/ui/Modifier.kt").unwrap();
    let modifier_jar_source = r#"
package androidx.compose.ui

interface Modifier {
    infix fun then(other: Modifier): Modifier =
        if (other === Modifier) this else CombinedModifier(this, other)

    companion object : Modifier {
        override infix fun then(other: Modifier): Modifier = other
    }
}

class CombinedModifier(val outer: Modifier, val inner: Modifier) : Modifier
"#;
    index.index_source(modifier_jar_url.clone(), modifier_jar_source);
    index.sources.remove(&modifier_jar_url);

    let size_jar_url = Url::parse(
        "jar:file:///caches/foundation-layout-sources.jar!/androidx/compose/foundation/layout/Size.kt",
    )
    .unwrap();
    let size_jar_source = r#"
package androidx.compose.foundation.layout

import androidx.compose.ui.Modifier

class Density
private class SizeModifier : Modifier

fun Modifier.width(width: Density.() -> Int) =
    this.then(
        SizeModifier(
            minWidth = width,
            maxWidth = width,
            enforceIncoming = true,
            inspectorInfo =
                debugInspectorInfo {
                    name = "width"
                    value = width
                }
        )
    )

fun Modifier.fillMaxWidth(fraction: Float = 1f) =
    this.then(SizeModifier())

fun defaultModifier(base: Modifier) =
    base.fillMaxWidth()
"#;
    index.index_source(size_jar_url.clone(), size_jar_source);
    index.sources.remove(&size_jar_url);
    index
}

#[test]
fn source_jar_multiline_single_expression_extension_and_member_return_type() {
    let mut index = setup_compose_source_jar_index();
    let app_url = Url::parse("file:///project/Screen.kt").unwrap();
    let app_code = r#"
package com.example.app

import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.defaultModifier

fun Screen(mod: Modifier) {
    val sizedWithLambda = mod.width { 100 }
    val filled = mod.fillMaxWidth()
    val chained = mod.width { 100 }.fillMaxWidth()
    val fromTopLevel = defaultModifier(mod)
    val x = 1
}
"#;
    index.index_source(app_url.clone(), app_code);

    let expected_modifier = Some(KotlinType::simple_class("androidx.compose.ui.Modifier"));
    let pos = Position::new(13, 4);

    assert_eq!(
        infer_variable_type(&index, &app_url, "sizedWithLambda", pos, app_code, None),
        expected_modifier,
        "Higher-order multiline single-expression extension in -sources.jar should infer Modifier"
    );
    assert_eq!(
        infer_variable_type(&index, &app_url, "filled", pos, app_code, None),
        expected_modifier,
        "Regular multiline single-expression extension in -sources.jar should infer Modifier"
    );
    assert_eq!(
        infer_variable_type(&index, &app_url, "chained", pos, app_code, None),
        expected_modifier,
        "Chained calls on multiline single-expression extensions in -sources.jar should infer Modifier"
    );
    assert_eq!(
        infer_variable_type(&index, &app_url, "fromTopLevel", pos, app_code, None),
        expected_modifier,
        "Top-level multiline single-expression function in -sources.jar should infer Modifier"
    );
}
