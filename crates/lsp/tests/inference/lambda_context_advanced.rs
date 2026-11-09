use lsp::indexer::WorkspaceIndex;
use lsp::inference::{KotlinType, infer_variable_type};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn infer_lambda_from_typed_arg() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Nested.kt").unwrap();
    let code = r#"
package com.example

class ListItemScope

class List<T>(val first: T)

class ListScope {
    fun items(
        count: Int,
        key: ((index: Int) -> Any)? = null,
        contentType: (index: Int) -> Any? = { null },
        itemContent: ListItemScope.(index: Int) -> Unit,
    ) {
        error("The method is not implemented")
    }
}

fun <T> listOf(vararg items: T): List<T> = List(items[0])

fun List(content: ListScope.() -> Unit) {
    content(ListScope())
}

inline fun <T> ListScope.items(
    items: List<T>,
    noinline key: ((item: T) -> Any)? = null,
    noinline contentType: (item: T) -> Any? = { null },
    crossinline content: ListItemScope.(item: T) -> Unit,
) = content(ListItemScope(), items.first)

fun main() {
    val list = listOf("A", "correction", "of")
    List {
        items(list) { println(it) }
        items(list) { item -> println(item) }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(34, 30);
    let infer1 = infer_variable_type(&index, &url, "it", pos1, code, None);
    assert_eq!(infer1, Some(KotlinType::STRING));

    let pos2 = Position::new(35, 38);
    let infer2 = infer_variable_type(&index, &url, "item", pos2, code, None);
    assert_eq!(infer2, Some(KotlinType::STRING));
}

#[test]
fn trailing_lambda_overload_rejects_missing_required_params() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UnpassedParams.kt").unwrap();
    let code = r#"
package com.example

fun process(a: String, required: Int, block: (Int) -> Unit) {}
fun process(a: String, opt: Boolean = true, block: (String) -> Unit) {}

fun main() {
    process("hello") {
        println(it)
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 16);
    let inferred = infer_variable_type(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn lambda_overload_rejects_violated_type_param_upper_bound() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UpperBoundOverload.kt").unwrap();
    let code = r#"
package com.example

interface Animal
class Rock
class Box<T>

fun <T : Animal> Box<T>.handle(block: (T) -> Unit) {}
fun <T> Box<T>.handle(block: (String) -> Unit) {}

fun main(box: Box<Rock>) {
    box.handle {
        println(it)
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(12, 16);
    let inferred = infer_variable_type(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}

#[test]
fn non_trailing_lambda_and_named_lambda_arg_inference() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NonTrailingAndNamedLambda.kt").unwrap();
    let code = r#"
package com.example

fun <T> compute(input: T, transform: (T) -> T): T = transform(input)
fun configure(count: Int, label: String = "default", block: (Int) -> Unit) {}

fun main() {
    compute("hello", { item ->
        println(item)
        item
    })
    configure(1, block = { num ->
        println(num)
    })
}
"#;
    index.index_source(url.clone(), code);

    let pos_item = Position::new(8, 16);
    let inferred_item = infer_variable_type(&index, &url, "item", pos_item, code, None);
    assert_eq!(inferred_item, Some(KotlinType::STRING));

    let pos_num = Position::new(12, 16);
    let inferred_num = infer_variable_type(&index, &url, "num", pos_num, code, None);
    assert_eq!(inferred_num, Some(KotlinType::INT));
}

#[test]
fn receiver_only_lambda_preserves_inner_lexical_it_binding() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LexicalItShadowing.kt").unwrap();
    let code = r#"
package com.example

class Scope
fun withScope(block: Scope.() -> Unit) {}
fun eachInt(block: (Int) -> Unit) {}

fun main() {
    eachInt {
        val it = "shadowed"
        withScope {
            println(it)
        }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 20);
    let inferred = infer_variable_type(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, Some(KotlinType::STRING));
}
