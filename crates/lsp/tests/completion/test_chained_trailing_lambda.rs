use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_completion_chained_collection_map_and_filter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ChainedCollectionTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun <R> map(transform: (T) -> R): List<R> = TODO()
    fun filter(predicate: (T) -> Boolean): List<T> = TODO()
    fun first(): T = TODO()
    fun count(): Int = TODO()
}

class Item(val name: String, val count: Int)

fun test(items: List<Item>) {
    items.map { it.name }.filter { it.length > 5 }.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(13, 51); // right after trailing dot
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"first".to_string()),
        "Chained map and filter should offer List<String> methods like 'first', got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"count".to_string()),
        "Chained map and filter should offer List<String> methods like 'count', got: {:?}",
        labels
    );
}

#[test]
fn test_completion_chained_multiline_trailing_lambdas() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/MultilineChainedTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun <R> map(transform: (T) -> R): List<R> = TODO()
    fun filter(predicate: (T) -> Boolean): List<T> = TODO()
    fun first(): T = TODO()
}

class Item(val name: String)

fun test(items: List<Item>) {
    items
        .map { it.name }
        .filter { it.length > 0 }
        .
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(15, 9); // on the trailing dot line
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"first".to_string()),
        "Multiline chained calls should offer List<String> methods, got: {:?}",
        labels
    );
}

#[test]
fn test_completion_chained_trailing_lambdas_with_terminal_method() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TerminalCallTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun <R> map(transform: (T) -> R): List<R> = TODO()
    fun filter(predicate: (T) -> Boolean): List<T> = TODO()
    fun first(): T = TODO()
}

class String {
    val length: Int = TODO()
    fun uppercase(): String = TODO()
}

class Item(val name: String, val active: Boolean)

fun test(items: List<Item>) {
    items.filter { it.active }.map { it.name }.first().
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(17, 56);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"length".to_string()),
        "Chained lambdas followed by first() should offer String property 'length', got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"uppercase".to_string()),
        "Chained lambdas followed by first() should offer String method 'uppercase', got: {:?}",
        labels
    );
}

#[test]
fn test_completion_chained_trailing_lambdas_with_explicit_parameters() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ExplicitParamsTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun <R> map(transform: (T) -> R): List<R> = TODO()
    fun filter(predicate: (T) -> Boolean): List<T> = TODO()
    fun first(): T = TODO()
}

class Item(val name: String)

fun test(items: List<Item>) {
    items.map { item -> item.name }.filter { name -> name.length > 0 }.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(12, 72);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"first".to_string()),
        "Chained lambdas with explicit parameters should offer List<String> methods, got: {:?}",
        labels
    );
}

#[test]
fn test_completion_chained_scope_functions_let_and_also() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ScopeFunctionChainTest.kt").unwrap();
    let source = r#"
package com.example

class String {
    val length: Int = TODO()
    fun uppercase(): String = TODO()
}

class User(val name: String)

fun test(user: User) {
    user.let { it.name }.also { println(it) }.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(11, 46);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"length".to_string()),
        "Chained let and also on User should resolve to String and offer 'length', got: {:?}",
        labels
    );
}

#[test]
fn test_completion_chained_trailing_lambda_with_partial_prefix() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/PartialPrefixTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun <R> map(transform: (T) -> R): List<R> = TODO()
    fun filter(predicate: (T) -> Boolean): List<T> = TODO()
    fun isEmpty(): Boolean = TODO()
}

class Item(val name: String)

fun test(items: List<Item>) {
    items.map { it.name }.filter { it.length > 5 }.isEmp
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(12, 56);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"isEmpty".to_string()),
        "Partial prefix matching 'isEmp' after chained lambdas should offer 'isEmpty', got: {:?}",
        labels
    );
}

#[test]
fn test_completion_chained_labeled_trailing_lambdas() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/LabeledLambdaTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun <R> map(transform: (T) -> R): List<R> = TODO()
    fun filter(predicate: (T) -> Boolean): List<T> = TODO()
    fun first(): T = TODO()
}

class Item(val name: String)

fun test(items: List<Item>) {
    items.map mapLabel@{ it.name }.filter filterLabel@{ it.length > 5 }.
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(12, 73);
    let completions = find_completions(&index, &url, pos, source);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"first".to_string()),
        "Chained labeled lambdas should offer List<String> methods like 'first', got: {:?}",
        labels
    );
}
