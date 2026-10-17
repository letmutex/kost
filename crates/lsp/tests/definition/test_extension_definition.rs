use lsp::features::find_definition;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_definition_resolves_extension_function_on_generic_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionDefinitionTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    val size: Int = TODO()
}

fun <T, R> List<T>.map(transform: (T) -> R): List<R> = TODO()

fun test(items: List<String>) {
    items.map { it.length }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(10, 11);
    let def = find_definition(&index, &url, "map", pos, source);
    assert!(
        def.is_some(),
        "Definition for extension 'map' should be found"
    );
    assert_eq!(def.unwrap().uri, url);
}

#[test]
fn test_definition_resolves_supertype_extension_function_across_files() {
    let mut index = WorkspaceIndex::new();

    let stdlib_url = Url::parse("file:///stdlib/Collections.kt").unwrap();
    let stdlib_source = r#"
package kotlin.collections

interface Iterable<out T>
interface Collection<out E> : Iterable<E>
interface List<out E> : Collection<E> {
    val size: Int
}

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> Iterable<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
"#;
    index.index_source(stdlib_url.clone(), stdlib_source);

    let user_url = Url::parse("file:///project/UserCode.kt").unwrap();
    let user_source = r#"
package com.example.app

fun process(list: List<String>) {
    list.map { it }
}
"#;
    index.index_source(user_url.clone(), user_source);

    let pos = Position::new(4, 10);
    let def = find_definition(&index, &user_url, "map", pos, user_source);
    assert!(
        def.is_some(),
        "Definition for supertype extension 'map' from stdlib should be found"
    );
    assert_eq!(def.unwrap().uri, stdlib_url);
}

#[test]
fn test_definition_resolves_extension_function_on_chained_trailing_lambda() {
    let mut index = WorkspaceIndex::new();

    let stdlib_url = Url::parse("file:///stdlib/Collections.kt").unwrap();
    let stdlib_source = r#"
package kotlin.collections

interface Iterable<out T>
interface Collection<out E> : Iterable<E>
interface List<out E> : Collection<E> {
    val size: Int
}

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> Iterable<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
"#;
    index.index_source(stdlib_url.clone(), stdlib_source);

    let user_url = Url::parse("file:///project/UserCode.kt").unwrap();
    let user_source = r#"
package com.example.app

class User(val name: String)

fun process(list: List<User>) {
    list.map { it.name }.filter { it.isNotEmpty() }
}
"#;
    index.index_source(user_url.clone(), user_source);

    let pos = Position::new(6, 27);
    let def = find_definition(&index, &user_url, "filter", pos, user_source);
    assert!(
        def.is_some(),
        "Definition for chained extension 'filter' should be found"
    );
    assert_eq!(def.unwrap().uri, stdlib_url);
}
