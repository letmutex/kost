use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::{KotlinType, TypeArg};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn generic_call_callable_reference_argument() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CallableRef.kt").unwrap();
    let code = r#"
package test.generic

class Stream<T>
fun <T1, T2, R> Stream<T1>.combine(other: Stream<T2>, transform: (T1, T2) -> R): Stream<R> = TODO()

class Text {
    fun append(other: Int): Text = TODO()
}

fun test(s1: Stream<Text>, s2: Stream<Int>) {
    val result = s1.combine(s2, Text::append)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 8);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.Stream",
        vec![TypeArg::invariant(KotlinType::simple_class(
            "test.generic.Text",
        ))],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn companion_and_object_callable_reference_bound_parameter_arity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CompanionCallableRef.kt").unwrap();
    let code = r#"
package test.generic

class Stream<T>
fun <T, R> Stream<T>.mapItems(transform: (T) -> R): Stream<R> = TODO()

class Service {
    companion object {
        fun fromId(id: Int): Service = TODO()
    }
}

object Parser {
    fun parse(raw: String): Int = 0
}

fun test(ids: Stream<Int>, raws: Stream<String>) {
    val services = ids.mapItems(Service::fromId)
    val parsed = raws.mapItems(Parser::parse)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(19, 4);
    let expected_services = KotlinType::class(
        "test.generic.Stream",
        vec![TypeArg::invariant(KotlinType::simple_class(
            "test.generic.Service",
        ))],
    );
    assert_eq!(
        infer_variable_type(&index, &url, "services", pos, code, None),
        Some(expected_services)
    );

    let expected_parsed = KotlinType::class(
        "test.generic.Stream",
        vec![TypeArg::invariant(KotlinType::INT)],
    );
    assert_eq!(
        infer_variable_type(&index, &url, "parsed", pos, code, None),
        Some(expected_parsed)
    );
}

#[test]
fn extension_property_callable_reference_type_binding() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtPropCallableRef.kt").unwrap();
    let code = r#"
package test.generic

class Box<T>
class Stream<T>
val <T> Box<T>.unwrapped: T get() = TODO()
fun <T, R> Stream<T>.mapItems(transform: (T) -> R): Stream<R> = TODO()

fun test(boxes: Stream<Box<String>>) {
    val strings = boxes.mapItems(Box::unwrapped)
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 4);
    let expected = KotlinType::class(
        "test.generic.Stream",
        vec![TypeArg::invariant(KotlinType::STRING)],
    );
    assert_eq!(
        infer_variable_type(&index, &url, "strings", pos, code, None),
        Some(expected)
    );
}
