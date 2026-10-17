use lsp::indexer::WorkspaceIndex;
use lsp::inference::{
    KotlinType, TypeArg, infer_receiver_type, infer_scope_receiver, infer_variable_type,
};
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_infer_lambda_nested() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Nested.kt").unwrap();
    let code = r#"
package com.example.nested

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class Post(val title: String)
class User(val name: String, val posts: List<Post>)

fun process(users: List<User>) {
    users.forEach { u ->
        u.posts.forEach { p ->
            val post = p
        }
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(13, 22);
    let inferred = infer_variable_type(&index, &url, "p", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.nested.Post"));
}

#[test]
fn test_infer_lambda_receiver_type_for_completion() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Completion.kt").unwrap();
    let code = r#"
package com.example.compl

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun process(users: List<User>) {
    users.forEach { u ->
        u.
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(11, 10);
    let inferred = infer_receiver_type(&index, &url, "u", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.compl.User"));
}

#[test]
fn test_infer_lambda_param_generic_extension_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtGeneric.kt").unwrap();
    let code = r#"
package com.example.ext

class User(val name: String, val age: Int)
class List<T>

fun <T> List<T>.filterItems(predicate: (T) -> Boolean): List<T> {
    return this
}

fun process(users: List<User>) {
    users.filterItems { u ->
        val user = u
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(12, 16);
    let inferred = infer_variable_type(&index, &url, "u", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.ext.User"));
}

#[test]
fn test_infer_lambda_implicit_it_generic_extension_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtIt.kt").unwrap();
    let code = r#"
package com.example.ext

class User(val name: String, val age: Int)
class List<T>

fun <T> List<T>.forEachCustom(action: (T) -> Unit) {}

fun process(users: List<User>) {
    users.forEachCustom {
        val user = it
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 16);
    let inferred = infer_scope_receiver(&index, &url, "it", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.ext.User"));
}

#[test]
fn test_infer_lambda_param_cross_file_extension_function() {
    let mut index = WorkspaceIndex::new();
    let decl_url = Url::parse("file:///project/ExtDecl.kt").unwrap();
    let decl_code = r#"
package com.example.ext

class Order(val id: Long)

fun Order.handle(callback: (Order, String) -> Unit) {}
"#;
    index.index_source(decl_url.clone(), decl_code);

    let usage_url = Url::parse("file:///project/ExtUsage.kt").unwrap();
    let usage_code = r#"
package com.example.ext

fun run(order: Order) {
    order.handle { ord, msg ->
        val o = ord
        val m = msg
    }
}
"#;
    index.index_source(usage_url.clone(), usage_code);

    let pos_ord = Position::new(5, 16);
    let ord_type = infer_variable_type(&index, &usage_url, "ord", pos_ord, usage_code, None);
    assert_eq!(ord_type, KotlinType::parse("com.example.ext.Order"));

    let pos_msg = Position::new(6, 16);
    let msg_type = infer_variable_type(&index, &usage_url, "msg", pos_msg, usage_code, None);
    assert_eq!(msg_type, KotlinType::parse("String"));
}

#[test]
fn test_infer_trailing_lambda_param_disambiguation_by_arity() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ScaffoldTest.kt").unwrap();
    let code = r#"
object Modifier
object PaddingValues

@Composable
fun Scaffold(
    modifier: Modifier = Modifier,
    topBar: @Composable () -> Unit = {},
    content: @Composable (PaddingValues) -> Unit,
) {}

fun test() {
    Scaffold { padding -> }
    Scaffold() { padding -> }
}
"#;
    index.index_source(url.clone(), code);

    let pos1 = Position::new(12, 15);
    let inferred1 = infer_variable_type(&index, &url, "padding", pos1, code, None);
    assert_eq!(inferred1, KotlinType::parse("PaddingValues"));

    let pos2 = Position::new(13, 17);
    let inferred2 = infer_variable_type(&index, &url, "padding", pos2, code, None);
    assert_eq!(inferred2, KotlinType::parse("PaddingValues"));
}

#[test]
fn test_chained_trailing_lambda_with_intervening_comment_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CommentChain.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()

fun test() {
    val list = listOf(1, 2, 3)
    val list1 = list /* comment */ .map { "test" }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let inferred = infer_variable_type(&index, &url, "list1", pos, code, None);
    assert_eq!(inferred, KotlinType::parse("com.example.coll.List<String>"));
}

#[test]
fn test_chained_trailing_lambda_duplicate_bodies_unresolved_member_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DuplicateLambdas.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
val String.length: Int get() = 0

fun test() {
    val list = listOf(1, 2, 3)
    val list2 = list.map { "test" }
        .map { it.length }
        .map { it.length }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 8);
    let inferred = infer_variable_type(&index, &url, "list2", pos, code, None);
    let expected = Some(KotlinType::class(
        "com.example.coll.List",
        vec![TypeArg::invariant(KotlinType::type_param("R"))],
    ));
    assert_eq!(inferred, expected);
}

#[test]
fn test_chained_trailing_lambda_duplicate_bodies_success_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/DuplicateLambdasSuccess.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
val String.length: Int get() = 0

fun test() {
    val list = listOf("a", "bb")
    val list3 = list.map { it.length }
        .map { it + 1 }
        .map { it + 1 }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(10, 8);
    let inferred = infer_variable_type(&index, &url, "list3", pos, code, None);
    let expected = Some(KotlinType::class(
        "com.example.coll.List",
        vec![TypeArg::invariant(KotlinType::INT)],
    ));
    assert_eq!(inferred, expected);
}

#[test]
fn test_chained_trailing_annotated_lambda_duplicate_bodies_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/AnnotatedLambdas.kt").unwrap();
    let code = r#"
package com.example.coll

annotation class Transform

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
val String.length: Int get() = 0

fun test() {
    val list = listOf("a", "bb")
    val list4 = list.map @Transform { it.length }
        .map @Transform { it + 1 }
        .map @Transform { it + 1 }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(12, 8);
    let inferred = infer_variable_type(&index, &url, "list4", pos, code, None);
    let expected = Some(KotlinType::class(
        "com.example.coll.List",
        vec![TypeArg::invariant(KotlinType::INT)],
    ));
    assert_eq!(inferred, expected);
}

#[test]
fn test_nested_statement_in_duplicate_lambda_type_infer() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/StatementsInLambda.kt").unwrap();
    let code = r#"
package com.example.coll

class List<T>
fun <T> listOf(vararg elements: T): List<T> = List()
fun <T, R> List<T>.map(transform: (T) -> R): List<R> = List()
val String.length: Int get() = 0

fun test() {
    val list = listOf("hello")
    val list5 = list.map {
        val item = it
        item.length
    }.map {
        val item = it
        item + 10
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos_first = Position::new(11, 12);
    let inferred_first = infer_variable_type(&index, &url, "item", pos_first, code, None);
    assert_eq!(inferred_first, Some(KotlinType::STRING));

    let pos_second = Position::new(14, 12);
    let inferred_second = infer_variable_type(&index, &url, "item", pos_second, code, None);
    assert_eq!(inferred_second, Some(KotlinType::INT));
}
