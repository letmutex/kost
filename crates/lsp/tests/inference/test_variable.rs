use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_variable_from_constructor() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Main.kt").unwrap();
    let code = r#"
package com.example.main

class User(val name: String)

fun test() {
    val user = User("Bob")
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "user", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.main.User"));
}

#[test]
fn test_infer_variable_from_function_call() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class User(val id: String, val name: String)
fun createUser(): User = User("1", "Alice")
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.createUser

fun test() {
    val user = createUser()
    val x = 1
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &main_url, "user", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.model.User"));
}

#[test]
fn test_infer_variable_from_method_call() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

class Profile(val bio: String)
class AccountService {
    fun getProfile(): Profile = Profile("bio")
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.service.AccountService

fun test(service: AccountService) {
    val profile = service.getProfile()
    val x = 1
}
"#;
    index.index_source(service_url, service_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &main_url, "profile", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.service.Profile"));
}

#[test]
fn test_infer_variable_from_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CastTest.kt").unwrap();
    let code = r#"
package com.example.cast

interface Vehicle
class Car(val horsepower: Int) : Vehicle
"#;
    index.index_source(url.clone(), code);

    let code_fn = r#"
package com.example.cast

interface Vehicle
class Car(val horsepower: Int) : Vehicle

fun test(v: Vehicle) {
    val car = v as Car
    val x = 1
}
"#;
    index.index_source(url.clone(), code_fn);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "car", pos, code_fn, None);
    assert_eq!(inferred, KotlinType::parse("com.example.cast.Car"));
}

#[test]
fn test_infer_variable_from_safe_cast() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SafeCastTest.kt").unwrap();
    let code = r#"
package com.example.cast

interface Worker
class Engineer(val lang: String) : Worker

fun test(w: Worker) {
    val eng = w as? Engineer
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 8);
    let inferred = infer_variable_type(&index, &url, "eng", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.cast.Engineer?"));
}

#[test]
fn test_infer_variable_from_cast_with_trailing_comment() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CastCommentTest.kt").unwrap();
    let code = r#"
package com.example.cast

interface Worker
class Engineer(val lang: String) : Worker

fun test(w: Worker) {
    val eng = w as Engineer // inline comment
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 8);
    let inferred = infer_variable_type(&index, &url, "eng", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.cast.Engineer"));
}

#[test]
fn test_nested_cast_argument_does_not_hijack_call_return_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedCastTest.kt").unwrap();
    let code = r#"
package com.example.cast

interface Worker
class Engineer(val lang: String) : Worker
class Report(val summary: String)

fun generateReport(w: Worker): Report = Report("done")

fun test(w: Worker) {
    val rep = generateReport(w as Engineer)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 8);
    let inferred = infer_variable_type(&index, &url, "rep", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.cast.Report"));
}

#[test]
fn test_infer_variable_from_cast_with_suspend_function_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SuspendCastTest.kt").unwrap();
    let code = r#"
package com.example.cast

fun test(obj: Any) {
    val handler = obj as suspend () -> Unit
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 8);
    let inferred = infer_variable_type(&index, &url, "handler", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("suspend () -> Unit"));
}

#[test]
fn test_infer_variable_from_multiline_parenthesized_cast_with_leading_comment() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultilineCastTest.kt").unwrap();
    let code = r#"
package com.example.cast

interface Worker
class Engineer(val lang: String) : Worker

fun test(w: Worker) {
    val eng = (/* note */ w as
        Engineer)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "eng", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.cast.Engineer"));
}

#[test]
fn test_infer_variable_from_alias_chain() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AliasTest.kt").unwrap();
    let code = r#"
package com.example.alias

class Config(val timeout: Int)

fun test() {
    val c1 = Config(30)
    val c2 = c1
    val c3 = c2
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "c3", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.alias.Config"));
}

#[test]
fn test_infer_variable_explicit_annotation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AnnotTest.kt").unwrap();
    let code = r#"
package com.example.annot

fun test(name: String) {
    val count: Int = 42
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 8);
    assert_eq!(
        infer_variable_type(&index, &url, "name", pos, code, None),
        KotlinType::parse("String")
    );
    assert_eq!(
        infer_variable_type(&index, &url, "count", pos, code, None),
        KotlinType::parse("Int")
    );
}

#[test]
fn test_generic_call_with_suspend_lambda_value_and_result_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Convert.kt").unwrap();
    let code = r#"public fun <T, R> convert(
    value: T,
    transform: suspend (value: T) -> R,
): R = TODO()

val result = convert("") { value -> 1 }
"#;
    index.index_source(url.clone(), code);

    let pos_result = Position::new(5, 4);
    let result_type = infer_variable_type(&index, &url, "result", pos_result, code, None);
    assert_eq!(result_type, Some(KotlinType::INT));

    let pos_value = Position::new(5, 28);
    let value_type = infer_variable_type(&index, &url, "value", pos_value, code, None);
    assert_eq!(value_type, Some(KotlinType::STRING));
}

#[test]
fn test_local_variable_nested_block_scope_resolution() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedScope.kt").unwrap();
    let code = r#"
fun testNestedScope() {
    val outer = "hello"
    run {
        val inner = 42
        run {
            val usage = outer
            val num = inner
        }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 24);
    let outer_type = infer_variable_type(&index, &url, "outer", pos, code, None);
    assert_eq!(outer_type, Some(KotlinType::STRING));

    let inner_type = infer_variable_type(&index, &url, "inner", pos, code, None);
    assert_eq!(inner_type, Some(KotlinType::INT));
}
