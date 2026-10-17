use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_member_completion_includes_top_level_extension_functions() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExtensionCompletionTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    val size: Int = TODO()
    fun get(index: Int): T = TODO()
}

fun <T, R> List<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> List<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
fun <T> List<T>.first(): T = TODO()

class Item(val name: String)

fun test(items: List<Item>) {
    items.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(15, 10);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"size".to_string()),
        "Direct member 'size' should be present, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"map".to_string()),
        "Extension function 'map' should be included in member completions, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"filter".to_string()),
        "Extension function 'filter' should be included in member completions, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"first".to_string()),
        "Extension function 'first' should be included in member completions, got: {:?}",
        labels
    );
}

#[test]
fn test_member_completion_includes_extension_functions_on_supertypes() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SupertypeExtensionTest.kt").unwrap();
    let source = r#"
package com.example

interface Iterable<T>
interface Collection<T> : Iterable<T>
class List<T> : Collection<T> {
    val size: Int = TODO()
}

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> Iterable<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
fun <T> Collection<T>.isEmpty(): Boolean = TODO()

class Item(val name: String)

fun test(items: List<Item>) {
    items.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(16, 10);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"size".to_string()),
        "Member 'size' on List should be present, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"map".to_string()),
        "Extension 'map' on Iterable should be included for List receiver, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"filter".to_string()),
        "Extension 'filter' on Iterable should be included for List receiver, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"isEmpty".to_string()),
        "Extension 'isEmpty' on Collection should be included for List receiver, got: {:?}",
        labels
    );
}

#[test]
fn test_member_completion_on_chained_trailing_lambda_includes_extension_functions() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainedExtensionTest.kt").unwrap();
    let source = r#"
package com.example

interface Iterable<T>
class List<T> : Iterable<T> {
    val size: Int = TODO()
}

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> Iterable<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
fun <T> Iterable<T>.first(): T = TODO()

class Item(val name: String)

fun test(items: List<Item>) {
    items.map { it.name }.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(15, 26);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"filter".to_string()),
        "Chained extension map should return List<String> and offer extension 'filter', got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"first".to_string()),
        "Chained extension map should return List<String> and offer extension 'first', got: {:?}",
        labels
    );
}

#[test]
fn test_member_completion_with_stdlib_collections_indexed_in_separate_file() {
    let mut index = WorkspaceIndex::new();

    let stdlib_url = Url::parse("file:///stdlib/Collections.kt").unwrap();
    let stdlib_source = r#"
package kotlin.collections

interface Iterable<out T>
interface Collection<out E> : Iterable<E>
interface List<out E> : Collection<E> {
    val size: Int
    fun get(index: Int): E
    fun contains(element: @UnsafeVariance E): Boolean
}

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> Iterable<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
fun <T> Iterable<T>.forEach(action: (T) -> Unit): Unit = TODO()
fun <T> Collection<T>.isEmpty(): Boolean = TODO()
"#;
    index.index_source(stdlib_url, stdlib_source);

    let user_url = Url::parse("file:///project/UserCode.kt").unwrap();
    let user_source = r#"
package com.example.app

fun process(list: List<String>) {
    list.
}
"#;
    index.index_source(user_url.clone(), user_source);

    let pos = Position::new(4, 9);
    let completions = find_completions(&index, &user_url, pos, user_source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"size".to_string()),
        "Direct member 'size' should be present, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"map".to_string()),
        "Extension 'map' from stdlib should be present on list., got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"filter".to_string()),
        "Extension 'filter' from stdlib should be present on list., got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"forEach".to_string()),
        "Extension 'forEach' from stdlib should be present on list., got: {:?}",
        labels
    );
}

#[test]
fn test_member_completion_on_chained_map_with_stdlib_in_separate_file() {
    let mut index = WorkspaceIndex::new();

    let stdlib_url = Url::parse("file:///stdlib/Collections.kt").unwrap();
    let stdlib_source = r#"
package kotlin.collections

interface Iterable<out T>
interface Collection<out E> : Iterable<E>
interface List<out E> : Collection<E> {
    val size: Int
    fun get(index: Int): E
    fun contains(element: @UnsafeVariance E): Boolean
}

fun <T, R> Iterable<T>.map(transform: (T) -> R): List<R> = TODO()
fun <T> Iterable<T>.filter(predicate: (T) -> Boolean): List<T> = TODO()
fun <T> Iterable<T>.forEach(action: (T) -> Unit): Unit = TODO()
"#;
    index.index_source(stdlib_url, stdlib_source);

    let user_url = Url::parse("file:///project/UserCode.kt").unwrap();
    let user_source = r#"
package com.example.app

class User(val name: String, val age: Int)

fun process(list: List<User>) {
    list.map { it.name }.
}
"#;
    index.index_source(user_url.clone(), user_source);

    let pos = Position::new(6, 25);
    let completions = find_completions(&index, &user_url, pos, user_source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"filter".to_string()),
        "Extension 'filter' from stdlib should be present on list.map {{ it.name }}., got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"forEach".to_string()),
        "Extension 'forEach' from stdlib should be present on list.map {{ it.name }}., got: {:?}",
        labels
    );
}
