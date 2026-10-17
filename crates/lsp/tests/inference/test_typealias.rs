use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_private_typealias_not_visible_in_other_files() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/PrivateAlias.kt").unwrap();
    let file_a_code = r#"
package com.example.model

private typealias Id = Int
val myId: Id = 10
"#;
    let file_b_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let file_b_code = r#"
package com.example.model

class Id(val uuid: String)

fun test() {
    val obj = Id("abc")
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos_a = Position::new(4, 4);
    let inferred_a = infer_variable_type(&index, &file_a_url, "myId", pos_a, file_a_code, None);
    assert_eq!(inferred_a, KotlinType::parse("Int"));

    let pos_b = Position::new(6, 8);
    let inferred_b = infer_variable_type(&index, &file_b_url, "obj", pos_b, file_b_code, None);
    assert_eq!(inferred_b, KotlinType::parse("com.example.model.Id"));
}

#[test]
fn test_public_typealias_visible_across_files_in_same_package() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Types.kt").unwrap();
    let file_a_code = r#"
package com.example.model

typealias UserId = String
"#;
    let file_b_url = Url::parse("file:///project/User.kt").unwrap();
    let file_b_code = r#"
package com.example.model

val uid: UserId = "123"
"#;
    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos = Position::new(3, 4);
    let inferred = infer_variable_type(&index, &file_b_url, "uid", pos, file_b_code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_public_typealias_visible_via_explicit_import() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/ScoreType.kt").unwrap();
    let file_a_code = r#"
package com.example.types

typealias Score = Double
"#;
    let file_b_url = Url::parse("file:///project/Game.kt").unwrap();
    let file_b_code = r#"
package com.example.app

import com.example.types.Score

val s: Score = 99.5
"#;
    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &file_b_url, "s", pos, file_b_code, None);
    assert_eq!(inferred, KotlinType::parse("Double"));
}

#[test]
fn test_public_typealias_hidden_without_import() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/OtherToken.kt").unwrap();
    let file_a_code = r#"
package com.example.other

typealias Token = Int
"#;
    let file_b_url = Url::parse("file:///project/AppToken.kt").unwrap();
    let file_b_code = r#"
package com.example.app

class Token(val key: String)

fun test() {
    val t = Token("abc")
}
"#;
    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &file_b_url, "t", pos, file_b_code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.app.Token"));
}

#[test]
fn test_public_typealias_not_leaked_via_same_name_generic_param_in_scope() {
    let mut index = WorkspaceIndex::new();
    let file_other_url = Url::parse("file:///project/OtherToken.kt").unwrap();
    let file_other_code = r#"
package com.example.other

typealias Token = Int
"#;
    let file_app_url = Url::parse("file:///project/AppToken.kt").unwrap();
    let file_app_code = r#"
package com.example.app

class Container<Token>(val item: Token)

class Token(val key: String)

fun test() {
    val t = Token("abc")
}
"#;
    index.index_source(file_other_url, file_other_code);
    index.index_source(file_app_url.clone(), file_app_code);

    let pos = Position::new(8, 8);
    let inferred = infer_variable_type(&index, &file_app_url, "t", pos, file_app_code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.app.Token"));
}

#[test]
fn test_infer_generic_class_constructor_with_stdlib_typealias_collision() {
    let mut index = WorkspaceIndex::new();
    let stdlib_url = Url::parse("file:///stdlib/SequenceBuilder.kt").unwrap();
    let stdlib_code = r#"
package kotlin.collections

private typealias State = Int
"#;
    index.index_source(stdlib_url, stdlib_code);

    let url = Url::parse("file:///project/State.kt").unwrap();
    let code = r#"class State<T>(val value: T)
val state = State(2)
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(1, 4);
    let inferred = infer_variable_type(&index, &url, "state", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("State<Int>"));
}

#[test]
fn test_unimported_typealias_fallback_not_leaked_for_function_typealias() {
    let mut index = WorkspaceIndex::new();
    let other_url = Url::parse("file:///project/Other.kt").unwrap();
    let other_code = r#"
package com.other

typealias Handler = OtherAction
"#;
    index.index_source(other_url, other_code);

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.app

typealias Handler = () -> Unit

class Handler
val h = Handler()
"#;
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(6, 4);
    let inferred = infer_variable_type(&index, &app_url, "h", pos, app_code, None);
    assert_eq!(inferred, KotlinType::parse("com.app.Handler"));
}

#[test]
fn test_chained_typealias_resolves_transitively() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Chain.kt").unwrap();
    let code = r#"
package com.example.alias

typealias Inner = Int
typealias Middle = Inner
typealias Outer = Middle

val num: Outer = 42
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "num", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_public_typealias_visible_via_identity_import_alias() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Types.kt").unwrap();
    let file_a_code = r#"
package com.example.types

typealias UserId = String
"#;
    let file_b_url = Url::parse("file:///project/App.kt").unwrap();
    let file_b_code = r#"
package com.example.app

import com.example.types.UserId as UserId

val u: UserId = "123"
"#;
    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &file_b_url, "u", pos, file_b_code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_cyclic_typealias_terminates_gracefully() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Cycle.kt").unwrap();
    let code = r#"
package com.example.cycle

typealias AliasA = AliasB
typealias AliasB = AliasA

val item: AliasA = 10
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "item", pos, code, None);
    assert!(inferred.is_some());
}

#[test]
fn test_import_alias_does_not_make_unrelated_same_name_declaration_visible() {
    let mut index = WorkspaceIndex::new();
    let file_secret_url = Url::parse("file:///project/other/Secret.kt").unwrap();
    let file_secret_code = r#"
package com.other

class Secret(val value: Int)
"#;
    let file_unrelated_url = Url::parse("file:///project/unrelated/Renamed.kt").unwrap();
    let file_unrelated_code = r#"
package com.unrelated

class Renamed(val text: String)
"#;
    let file_app_url = Url::parse("file:///project/app/Main.kt").unwrap();
    let file_app_code = r#"
package com.app

import com.other.Secret as Renamed

val r = Renamed(10)
"#;
    index.index_source(file_secret_url, file_secret_code);
    index.index_source(file_unrelated_url, file_unrelated_code);
    index.index_source(file_app_url.clone(), file_app_code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &file_app_url, "r", pos, file_app_code, None);
    assert_eq!(inferred, KotlinType::parse("com.other.Secret"));
}

#[test]
fn test_generic_parameterized_typealias_expansion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericAlias.kt").unwrap();
    let code = r#"
package com.example.alias

class Map<K, V>

typealias StringMap<V> = Map<String, V>

val m: StringMap<Int> = Map()
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "m", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.alias.Map<String, Int>")
    );
}

#[test]
fn test_function_typealias_expansion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FunctionAlias.kt").unwrap();
    let code = r#"
package com.example.alias

typealias Handler<T> = (T) -> Unit

val fnHandler: Handler<String> = { s -> }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "fnHandler", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("(String) -> Unit"));
}
