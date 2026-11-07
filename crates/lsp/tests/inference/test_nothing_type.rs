use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::{KotlinType, TypeArg};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_throw_expression_evaluates_nothing() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ThrowNothing.kt").unwrap();
    let code = r#"
package test.nothing

class Container<out T>
fun <T, R> Container<T>.transform(f: (T) -> R): Container<R> = TODO()
class CustomError

fun test(container: Container<Int>) {
    val result = container.transform { throw CustomError() }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    let expected = KotlinType::class(
        "test.nothing.Container",
        vec![TypeArg::invariant(KotlinType::NOTHING)],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn test_elvis_nothing_rhs_narrowing() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ElvisNothing.kt").unwrap();
    let code = r#"
package test.nothing

class CustomException
fun getError(): CustomException? = null
fun fail(): Nothing = throw RuntimeException()

fun test() {
    val err = getError() ?: fail()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "err", pos, code, None);
    let expected = KotlinType::simple_class("test.nothing.CustomException");
    assert_eq!(inferred, Some(expected));
}

#[test]
fn test_elvis_operator_with_void_rhs() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ElvisVoid.kt").unwrap();
    let code = r#"
package test.nothing

class CustomException
fun getError(): CustomException? = null
fun fail(): Void = TODO()

fun test() {
    val err = getError() ?: fail()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 8);
    let inferred = infer_variable_type(&index, &url, "err", pos, code, None);
    let expected = KotlinType::simple_class("test.nothing.CustomException");
    assert_eq!(inferred, Some(expected));
}
