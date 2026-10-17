use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_scope_receiver, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_lambda_param_collection_foreach() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ForEach.kt").unwrap();
    let code = r#"
package com.example.test

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String, val age: Int)

fun process(users: List<User>) {
    users.forEach { u ->
        val x = u
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 16);
    let inferred = infer_variable_type(&index, &url, "u", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.test.User"));
}

#[test]
fn test_infer_lambda_param_collection_indexed() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Indexed.kt").unwrap();
    let code = r#"
package com.example.test

class List<T> {
    fun forEachIndexed(action: (Int, T) -> Unit) {}
}

class User(val name: String)

fun process(users: List<User>) {
    users.forEachIndexed { idx, u ->
        val i = idx
        val user = u
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 16);
    let idx_type = infer_variable_type(&index, &url, "idx", pos, code, None);
    assert_eq!(idx_type, KotlinType::parse("Int"));

    let u_type = infer_variable_type(&index, &url, "u", Position::new(12, 16), code, None);
    assert_eq!(u_type, KotlinType::parse("com.example.test.User"));
}

#[test]
fn test_infer_lambda_implicit_it_collection() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ImplicitIt.kt").unwrap();
    let code = r#"
package com.example.test

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class Item(val id: Long, val title: String)

fun process(items: List<Item>) {
    items.forEach {
        val i = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 16);
    let inferred = infer_scope_receiver(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.test.Item"));
}

#[test]
fn test_infer_lambda_param_custom_higher_order_method() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Callback.kt").unwrap();
    let code = r#"
package com.example.event

class Event(val id: String, val timestamp: Long)

class EventManager {
    fun setListener(listener: (Event, Int) -> Unit) {}
}

fun setup(manager: EventManager) {
    manager.setListener { evt, status ->
        val e = evt
        val s = status
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos_evt = Position::new(11, 16);
    let evt_type = infer_variable_type(&index, &url, "evt", pos_evt, code, None);
    assert_eq!(evt_type, KotlinType::parse("com.example.event.Event"));

    let pos_status = Position::new(12, 16);
    let status_type = infer_variable_type(&index, &url, "status", pos_status, code, None);
    assert_eq!(status_type, KotlinType::parse("Int"));
}

#[test]
fn test_infer_lambda_param_generic_method() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericObs.kt").unwrap();
    let code = r#"
package com.example.rx

class Message(val payload: String)

class Observable<T> {
    fun subscribe(consumer: (T) -> Unit) {}
}

fun test(obs: Observable<Message>) {
    obs.subscribe { msg ->
        val m = msg
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 16);
    let inferred = infer_variable_type(&index, &url, "msg", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.rx.Message"));
}

#[test]
fn test_infer_lambda_param_toplevel_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TopLevel.kt").unwrap();
    let code = r#"
package com.example.toplevel

class Session(val token: String)

fun runWithSession(block: (Session) -> Unit) {}

fun test() {
    runWithSession { s ->
        val sess = s
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 18);
    let inferred = infer_variable_type(&index, &url, "s", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.toplevel.Session"));
}

#[test]
fn test_infer_lambda_implicit_it_negative_when_explicit_params() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NegativeIt.kt").unwrap();
    let code = r#"
package com.example.test

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun process(users: List<User>) {
    users.forEach { u ->
        val x = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 16);
    let inferred = infer_scope_receiver(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, None);
}

#[test]
fn test_infer_trailing_lambda_with_preceding_arguments_infers_parameter_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TrailingArgs.kt").unwrap();
    let code = r#"
package com.example.test

fun <T, R> transform(tag: String, item: T, block: (T) -> R): R = block(item)

fun test() {
    transform("tag", 42) { num ->
        val x = num
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 16);
    let inferred = infer_variable_type(&index, &url, "num", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_call_with_argument_lambda_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ArgLambda.kt").unwrap();
    let code = r#"
package com.example.test

fun <T, R> process(item: T, init: () -> R): R = init()

fun test() {
    val res = process(10, { "start" })
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_single_type_argument_lambda_param_fallback() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SingleArgLambda.kt").unwrap();
    let code = r#"
package com.example.test

class Container<T> {
    fun iterate(action: (T) -> Unit) {}
}

fun test(c: Container<String>) {
    c.iterate { item ->
        val x = item
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 12);
    let inferred = infer_variable_type(&index, &url, "item", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn test_generic_suspend_lambda_implicit_it_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ConvertIt.kt").unwrap();
    let code = r#"
public fun <T, R> convert(
    value: T,
    transform: suspend (T) -> R,
): R = TODO()

val result = convert("hello") { it }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 32);
    let it_type = infer_scope_receiver(&index, &url, "it", pos, code, None);
    assert_eq!(it_type, Some(KotlinType::STRING));
}

#[test]
fn test_generic_suspend_lambda_multi_param_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultiParam.kt").unwrap();
    let code = r#"
public fun <K, V, R> transformMap(
    key: K,
    value: V,
    action: suspend (k: K, v: V) -> R,
): R = TODO()

val res = transformMap("key", 42) { k, v -> 1 }
"#;
    index.index_source(url.clone(), code);

    let pos_k = Position::new(7, 36);
    let k_type = infer_variable_type(&index, &url, "k", pos_k, code, None);
    assert_eq!(k_type, Some(KotlinType::STRING));

    let pos_v = Position::new(7, 39);
    let v_type = infer_variable_type(&index, &url, "v", pos_v, code, None);
    assert_eq!(v_type, Some(KotlinType::INT));
}

#[test]
fn test_functional_param_arity_mismatch_does_not_cross_bind() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ArityMismatch.kt").unwrap();
    let code = r#"
fun test(onSuccess: (String) -> Unit, onComplete: () -> Unit) {}

fun run() {
    test({ "ok" }) { err -> }
}
"#;
    index.index_source(url.clone(), code);

    let pos_err = Position::new(4, 22);
    let err_type = infer_variable_type(&index, &url, "err", pos_err, code, None);
    assert_eq!(err_type, None);
}
