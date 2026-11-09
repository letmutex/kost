use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::{KotlinType, TypeArg};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn explicit_type_arg_precedence() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericPrecedence.kt").unwrap();
    let code = r#"
package test.generic

class Task<T>
class CustomError

fun <T> execute(block: () -> T): Task<T> = Task()

fun test() {
    val task = execute<Unit> { throw CustomError() }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 4);
    let inferred = infer_variable_type(&index, &url, "task", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.Task",
        vec![TypeArg::invariant(KotlinType::UNIT)],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn builder_clause_return_unification() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BuilderClause.kt").unwrap();
    let code = r#"
package test.generic

interface ScopeBuilder<in R> {
    fun clause(action: () -> R)
}
inline fun <R> buildScope(builder: ScopeBuilder<R>.() -> Unit): R = TODO()
class EventTrigger {
    fun onEvent(action: () -> String) {}
}

fun test(trigger: EventTrigger) {
    val result = buildScope {
        trigger.onEvent { "OK" }
    }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(15, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn type_arg_count_selects_overload() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TypeArgCount.kt").unwrap();
    let code = r#"
package test.generic

class Engine
data class Params(val url: String)

fun <R> Engine.define(name: String, block: (args: Array<Any?>) -> R) {}
fun <T, R> Engine.define(name: String, block: (T) -> R) {}

fun test(engine: Engine) {
    engine.define<Params, String>("fetch") { args ->
        val (url) = args
        url
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 13);
    let inferred = infer_variable_type(&index, &url, "url", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn builder_nested_lambda_receiver_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/BuilderNested.kt").unwrap();
    let code = r#"
package test.generic

interface ProducerScope<E> {
    fun send(element: E)
}

class Channel<E>
fun <E> produce(block: ProducerScope<E>.() -> Unit): Channel<E> = TODO()

class Container<T>
fun <T> Container<T>.forEach(action: (T) -> Unit) {}

fun test() {
    val result = produce {
        (1..10).forEach { send(it) }
    }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(14, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.Channel",
        vec![TypeArg::invariant(KotlinType::INT)],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn chained_call_after_trailing_lambda() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainedCall.kt").unwrap();
    let code = r#"
package test.generic

class Stream<T>
fun <T> Stream<T>.filterItems(): Stream<T> = TODO()
fun <T> Stream<T>.filterItems(predicate: (T, T) -> Boolean): Stream<T> = TODO()
fun <T> Stream<T>.drain(): List<T> = TODO()
class List<out T>
fun fail(msg: String): Void = TODO()

fun test(s: Stream<Int>) {
    val result = s.filterItems { _, _ -> fail("error") }.drain()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.List",
        vec![TypeArg::invariant(KotlinType::INT)],
    );
    assert_eq!(inferred, Some(expected));
}
