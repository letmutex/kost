use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_generic_function_call_inferred_args() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericCall.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>

fun <T> listOf(vararg elements: T): List<T> = List()

fun test() {
    val list1 = listOf(1, 2, 3)
    val list2 = listOf("a", "b")
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(9, 4);
    let inferred1 = infer_variable_type(&index, &url, "list1", pos1, code, None);
    assert_eq!(inferred1, KotlinType::parse("com.example.coll.List<Int>"));

    let pos2 = Position::new(10, 4);
    let inferred2 = infer_variable_type(&index, &url, "list2", pos2, code, None);
    assert_eq!(
        inferred2,
        KotlinType::parse("com.example.coll.List<String>")
    );
}

#[test]
fn test_infer_generic_function_call_explicit_type_args() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericExplicit.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>

fun <T> listOf(vararg elements: T): List<T> = List()

fun test() {
    val list = listOf<Int>(1, 2, 3)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "list", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.coll.List<Int>"));
}

#[test]
fn test_infer_generic_function_call_custom_class_arg() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericCustom.kt").unwrap();
    let code = r#"
package com.example.model

class User(val name: String)
class List<T>

fun <T> listOf(vararg elements: T): List<T> = List()

fun test() {
    val users = listOf(User("Alice"), User("Bob"))
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 4);
    let inferred = infer_variable_type(&index, &url, "users", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.model.List<com.example.model.User>")
    );
}

#[test]
fn test_infer_generic_function_multi_type_params() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/PairTest.kt").unwrap();
    let code = r#"
package com.example.util

class Pair<K, V>

fun <K, V> pairOf(first: K, second: V): Pair<K, V> = Pair()

fun test() {
    val p1 = pairOf("key", 100)
    val p2 = pairOf<String, Int>("key", 100)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(9, 4);
    let inferred1 = infer_variable_type(&index, &url, "p1", pos1, code, None);
    assert_eq!(
        inferred1,
        KotlinType::parse("com.example.util.Pair<String, Int>")
    );

    let pos2 = Position::new(10, 4);
    let inferred2 = infer_variable_type(&index, &url, "p2", pos2, code, None);
    assert_eq!(
        inferred2,
        KotlinType::parse("com.example.util.Pair<String, Int>")
    );
}

#[test]
fn test_infer_generic_member_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MemberCall.kt").unwrap();
    let code = r#"
package com.example.repo

class User(val name: String)

class Repository {
    fun <T> find(id: Int): T = null!!
    fun <T> save(entity: T): T = entity
}

fun test(repo: Repository, u: User) {
    val found = repo.find<User>(1)
    val saved = repo.save(u)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(12, 4);
    let inferred1 = infer_variable_type(&index, &url, "found", pos1, code, None);
    assert_eq!(inferred1, KotlinType::parse("com.example.repo.User"));

    let pos2 = Position::new(13, 4);
    let inferred2 = infer_variable_type(&index, &url, "saved", pos2, code, None);
    assert_eq!(inferred2, KotlinType::parse("com.example.repo.User"));
}

#[test]
fn test_infer_lambda_param_from_generic_call_result() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ForEachTest.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>

fun <T> listOf(vararg elements: T): List<T> = List()
fun <T> List<T>.forEach(action: (T) -> Unit): Unit {}

fun test() {
    val list1 = listOf(1, 2, 3)
    val list2 = listOf<Int>(1, 2, 3)
    list1.forEach { it ->
        val a = it
    }
    list2.forEach { it ->
        val b = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(12, 8);
    let inferred1 = infer_variable_type(&index, &url, "it", pos1, code, None);
    assert_eq!(inferred1, KotlinType::parse("Int"));

    let pos2 = Position::new(15, 8);
    let inferred2 = infer_variable_type(&index, &url, "it", pos2, code, None);
    assert_eq!(inferred2, KotlinType::parse("Int"));
}

#[test]
fn test_infer_generic_function_call_heterogeneous_args() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/HeteroCall.kt").unwrap();
    let code = r#"
package com.example.coll

class Any
class List<T>

fun <T> listOf(vararg elements: T): List<T> = List()

fun test() {
    val list3 = listOf(1, "2", false)
    val list4 = listOf(1, "2", false, Any())
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    // Register stdlib-like supertypes to simulate indexed bytecode
    index.class_supertypes.insert(
        "Int".to_string(),
        vec!["Comparable".to_string(), "Serializable".to_string()],
    );
    index.class_supertypes.insert(
        "String".to_string(),
        vec!["Comparable".to_string(), "Serializable".to_string()],
    );
    index.class_supertypes.insert(
        "Boolean".to_string(),
        vec!["Comparable".to_string(), "Serializable".to_string()],
    );

    let pos3 = Position::new(10, 4);
    let inferred3 = infer_variable_type(&index, &url, "list3", pos3, code, None);
    assert_eq!(inferred3, KotlinType::parse("com.example.coll.List<Any>"));

    let pos4 = Position::new(11, 4);
    let inferred4 = infer_variable_type(&index, &url, "list4", pos4, code, None);
    assert_eq!(inferred4, KotlinType::parse("com.example.coll.List<Any>"));
}

#[test]
fn test_infer_generic_function_call_common_supertype_args() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SupertypeCall.kt").unwrap();
    let code = r#"
package com.example.model

open class Animal
class Dog : Animal()
class Cat : Animal()
class List<T>

fun <T> listOf(vararg elements: T): List<T> = List()

fun test() {
    val animals = listOf(Dog(), Cat())
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 4);
    let inferred = infer_variable_type(&index, &url, "animals", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.model.List<com.example.model.Animal>")
    );
}

#[test]
fn test_infer_generic_class_constructor_and_member_access() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MyState.kt").unwrap();
    let code = r#"
package com.example.state

class MyState<T>(val value: T) {
    fun getValue2(): T {
        return value
    }
}

fun test() {
    val state = MyState<Int>(1)
    val value = state.value
    val value2 = state.getValue2()

    val state2 = MyState("1")
    val value3 = state2.value
    val value4 = state2.getValue2()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos_state = Position::new(10, 4);
    let inferred_state = infer_variable_type(&index, &url, "state", pos_state, code, None);
    assert_eq!(
        inferred_state,
        KotlinType::parse("com.example.state.MyState<Int>")
    );

    let pos_value = Position::new(11, 4);
    let inferred_value = infer_variable_type(&index, &url, "value", pos_value, code, None);
    assert_eq!(inferred_value, KotlinType::parse("Int"));

    let pos_value2 = Position::new(12, 4);
    let inferred_value2 = infer_variable_type(&index, &url, "value2", pos_value2, code, None);
    assert_eq!(inferred_value2, KotlinType::parse("Int"));

    let pos_state2 = Position::new(14, 4);
    let inferred_state2 = infer_variable_type(&index, &url, "state2", pos_state2, code, None);
    assert_eq!(
        inferred_state2,
        KotlinType::parse("com.example.state.MyState<String>")
    );

    let pos_value3 = Position::new(15, 4);
    let inferred_value3 = infer_variable_type(&index, &url, "value3", pos_value3, code, None);
    assert_eq!(inferred_value3, KotlinType::parse("String"));

    let pos_value4 = Position::new(16, 4);
    let inferred_value4 = infer_variable_type(&index, &url, "value4", pos_value4, code, None);
    assert_eq!(inferred_value4, KotlinType::parse("String"));
}

#[test]
fn test_infer_generic_class_constructor_type_param_matches_class_name() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/State.kt").unwrap();
    let code = r#"class State<State>(val value: State)
val state = State(2)
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(1, 4);
    let inferred = infer_variable_type(&index, &url, "state", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("State<Int>"));
}
