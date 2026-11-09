use lsp::indexer::WorkspaceIndex;
use lsp::inference::types::KotlinType;
use lsp::inference::variable::infer_variable_type;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn infer_extension_overload_bounded_receiver_specificity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionFormat.kt").unwrap();
    let code = r#"
fun <T> T.format(action: () -> Unit): Int = 0
fun <T : CharSequence> T.format(action: () -> Unit): String = ""

val formatted = "hello".format { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "formatted", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn infer_higher_order_call_rejects_missing_preceding_required_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionMissingParam.kt").unwrap();
    let code = r#"
class Text
fun <T> T.process(requiredHeader: String, action: () -> Unit): Int = 0

val result = Text().process { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, None);
}

#[test]
fn infer_higher_order_call_unit_coercion_for_functional_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionUnitCoercion.kt").unwrap();
    let code = r#"
class Text
class Block
fun <T> T.consume(action: () -> Unit): String = ""

val result = Text().consume { Block() }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(6, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("String"));
}

#[test]
fn infer_higher_order_call_default_unit_when_return_type_omitted() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionOmittedReturn.kt").unwrap();
    let code = r#"
class Text
fun <T> T.action(block: () -> Unit) { }

val res = Text().action { }
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(5, 4);
    let inferred = infer_variable_type(&index, &url, "res", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::UNIT));
}

#[test]
fn range_and_for_loop_builder_inference_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FlowBuilder.kt").unwrap();
    let code = r#"
package kotlinx.coroutines.flow

interface Flow<out T>
interface FlowCollector<in T> {
    suspend fun emit(value: T)
}

public fun <T> flow(block: suspend FlowCollector<T>.() -> Unit): Flow<T> = TODO()
public inline fun <T> Flow<T>.filter(crossinline predicate: suspend (T) -> Boolean): Flow<T> = this
public inline fun <T, R> Flow<T>.map(crossinline transform: suspend (T) -> R): Flow<R> = TODO()

class Benchmark {
    private fun numbers(limit: Long = 100L) = flow {
        for (i in 2L..limit) emit(i)
    }

    fun run() {
        val nums = numbers(1000L)
        val first = nums.filter { it % 2L != 0L }.map { it * it }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(19, 12);
    let inferred = infer_variable_type(&index, &url, "first", pos, code, None);
    assert_eq!(
        inferred,
        KotlinType::parse("kotlinx.coroutines.flow.Flow<Long>")
    );
}

#[test]
fn top_level_call_ignores_unimported_container_method_and_nested_enum() {
    let mut index = WorkspaceIndex::new();
    let junit_url = Url::parse("file:///project/Assert.kt").unwrap();
    let junit_code = r#"
package org.junit

class Assert {
    fun fail(): Unit {}
}
"#;
    index.index_source(junit_url, junit_code);

    let test_url = Url::parse("file:///project/Test.kt").unwrap();
    let test_code = r#"
package kotlin.test

fun fail(message: String? = null): Nothing = TODO()
"#;
    index.index_source(test_url, test_code);

    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package kotlinx.coroutines

import kotlin.test.*

fun check(err: Throwable?) {
    val exception = err ?: fail()
}
"#;
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 8);
    let inferred = infer_variable_type(&index, &main_url, "exception", pos, main_code, None);
    assert_eq!(inferred, KotlinType::parse("Throwable"));
}
