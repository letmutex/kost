use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, TypeArg, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_generic_function_call_from_lambda_body_return_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Remember.kt").unwrap();
    let code = r#"
package com.example.compose

class State(val count: Int)

@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun test() {
    val state = remember { State(0) }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let expr_inferred = lsp::inference::infer_expression_type(
        &index,
        &url,
        "remember { State(0) }",
        Position::default(),
        code,
        None,
    );
    assert_eq!(
        expr_inferred,
        KotlinType::parse("com.example.compose.State")
    );

    let pos = Position::new(15, 8);
    let inferred = infer_variable_type(&index, &url, "state", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.compose.State"));
}

#[test]
fn test_infer_generic_wrapper_call_from_lambda_body_return_type() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/RunCatching.kt").unwrap();
    let code = r#"
package com.example.result

class Result<out T>
class User(val name: String)

fun fetchUser(): User = User("Alice")

fun <R> runCatching(block: () -> R): Result<R> = Result()

fun test() {
    val res = runCatching { fetchUser() }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 4);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("com.example.result.Result<com.example.result.User>")
    );
}

#[test]
fn test_infer_collection_map_lambda_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Map.kt").unwrap();
    let code = r#"
package com.example.coll

class User(val name: String, val age: Int)
class List<T>

fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()

fun test(users: List<User>) {
    val names = users.map { it.name }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 4);
    let inferred = infer_variable_type(&index, &url, "names", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.coll.List<String>"));
}

#[test]
fn test_infer_collection_map_explicit_param() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MapExplicit.kt").unwrap();
    let code = r#"
package com.example.coll

class User(val name: String, val age: Int)
class List<T>

fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()

fun test(users: List<User>) {
    val ages = users.map { u -> u.age }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 4);
    let inferred = infer_variable_type(&index, &url, "ages", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.coll.List<Int>"));
}

#[test]
fn test_infer_scope_function_run_lambda_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Run.kt").unwrap();
    let code = r#"
package com.example.scope

class Config(val port: Int)

fun <R> run(block: () -> R): R = block()

fun test() {
    val cfg = run { Config(8080) }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "cfg", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.scope.Config"));
}

#[test]
fn test_infer_scope_function_let_lambda_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Let.kt").unwrap();
    let code = r#"
package com.example.scope

class User(val name: String, val age: Int)

fun <T, R> T.let(block: (T) -> R): R = block(this)

fun test(user: User) {
    val userAge = user.let { it.age }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "userAge", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_multi_statement_lambda_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultiStmt.kt").unwrap();
    let code = r#"
package com.example.multi

class Session(val token: String)

@Composable
inline fun <T> remember(
    key1: Any? = null,
    key2: Any? = null,
    key3: Any? = null,
    crossinline calculation: @DisallowComposableCalls () -> T,
): T = TODO()

fun test() {
    val sess = remember {
        val t = "abc"
        Session(t)
    }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(18, 4);
    let inferred = infer_variable_type(&index, &url, "sess", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.multi.Session"));
}

#[test]
fn test_infer_lambda_with_explicit_return_compound_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ReturnCompound.kt").unwrap();
    let code = r#"
package com.example.ret

fun <R> run(block: () -> R): R = block()

fun calculate(a: Int, b: Int): Int {
    val sum = run {
        return@run a + b
    }
    return sum
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &url, "sum", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Int"));
}

#[test]
fn test_infer_trailing_lambda_return_type_matching_functional_parameter_across_defaults_and_varargs()
 {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SaveableTest.kt").unwrap();
    let code = r#"
package com.example.saveable

class MutableState<T>(val value: T)
interface Saver<Original, Saveable>
fun <T, S> autoSaver(): Saver<T, S> = TODO()

fun <T : Any> rememberSaveable(
    vararg inputs: Any?,
    saver: Saver<T, out Any> = autoSaver(),
    key: String? = null,
    init: () -> T,
): T = TODO()

fun <T> mutableStateOf(value: T): MutableState<T> = TODO()

fun test() {
    val saveable = rememberSaveable { mutableStateOf(1) }
    val value = saveable.value
}
"#;
    index.index_source(url.clone(), code);

    let pos_saveable = Position::new(17, 8);
    let inferred_saveable = infer_variable_type(&index, &url, "saveable", pos_saveable, code, None);
    assert_eq!(
        inferred_saveable,
        KotlinType::parse("com.example.saveable.MutableState<Int>")
    );

    let pos_value = Position::new(18, 8);
    let inferred_value = infer_variable_type(&index, &url, "value", pos_value, code, None);
    assert_eq!(inferred_value, KotlinType::parse("Int"));
}

#[test]
fn test_infer_flow_extension_map_suspend_lambda_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Flow.kt").unwrap();
    let code = r#"
class Flow<T>(val value: T)

public inline fun <T, R> Flow<T>.map(
    crossinline transform: suspend (value: T) -> R,
): Flow<R> = TODO()

val flow = Flow(2).map { "" }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(7, 4);
    let inferred = infer_variable_type(&index, &url, "flow", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("Flow<String>"));
}

#[test]
fn test_chained_trailing_lambda_three_calls_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainThree.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
fun String.uppercase(): String = ""
val String.length: Int get() = 0

fun test() {
    val list = listOf(1, 2, 3)
    val list1 = list.map { "test" }.map { it.uppercase() }.map { it.length }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 8);
    let inferred = infer_variable_type(&index, &url, "list1", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.coll.List<Int>"));
}

#[test]
fn test_chained_trailing_lambda_invalid_member_on_primitive_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainInvalidMember.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
val String.length: Int get() = 0

fun test() {
    val list = listOf(1, 2, 3)
    val list1 = list.map { "test".length }.map { it.length }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "list1", pos, code, None);
    let expected = Some(KotlinType::class(
        "com.example.coll.List",
        vec![TypeArg::invariant(KotlinType::type_param("R"))],
    ));
    assert_eq!(inferred, expected);
}
