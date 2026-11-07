use lsp::indexer::WorkspaceIndex;
use lsp::inference::infer_variable_type;
use lsp::inference::types::{KotlinType, TypeArg};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_trailing_lambda_default_parameter_binding() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TrailingLambdaDefaults.kt").unwrap();
    let code = r#"
package test.generic

class Context
enum class Mode { DEFAULT }
class Task<T>

fun <T> execute(
    context: Context,
    mode: Mode = Mode.DEFAULT,
    block: () -> T
): Task<T> = TODO()

fun test(c: Context) {
    val first = execute(c) { "Hello" }
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(15, 4);
    let inferred = infer_variable_type(&index, &url, "first", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.Task",
        vec![TypeArg::invariant(KotlinType::STRING)],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn test_named_constructor_generic_type_binding() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NamedConstructor.kt").unwrap();
    let code = r#"
package test.generic

class Item
class Wrapper<T>(val data: T)

fun test() {
    val wrapped = Wrapper(data = Item())
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 4);
    let inferred = infer_variable_type(&index, &url, "wrapped", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.Wrapper",
        vec![TypeArg::invariant(KotlinType::simple_class(
            "test.generic.Item",
        ))],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn test_generic_call_nested_map_return() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/GenericMapReturn.kt").unwrap();
    let code = r#"
package test.generic

class Engine {
    fun <T> query(input: String): T = TODO()
}

fun test(engine: Engine) {
    val result = engine.query<Map<Any?, Any?>>("obj")
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "result", pos, code, None);
    let expected = KotlinType::class(
        "Map",
        vec![
            TypeArg::invariant(KotlinType::ANY.make_nullable()),
            TypeArg::invariant(KotlinType::ANY.make_nullable()),
        ],
    );
    assert_eq!(inferred, Some(expected));
}

#[test]
fn test_f_bounded_comparable_type_param_does_not_expand_recursively() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FBoundedComparable.kt").unwrap();
    let code = r#"
package test.generic

interface Comparable<T>
class List<out T>
fun <T : Comparable<T>> emptyComparableList(): List<T> = TODO()

fun test() {
    val items = emptyComparableList()
    val x = 1
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 4);
    let inferred = infer_variable_type(&index, &url, "items", pos, code, None);
    let expected = KotlinType::class(
        "test.generic.List",
        vec![TypeArg::invariant(KotlinType::type_param("T"))],
    );
    assert_eq!(inferred, Some(expected));
}
