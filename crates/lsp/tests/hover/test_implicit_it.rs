use lsp::config::VariableHoverTypeInference;
use lsp::features::{find_hover, find_hover_with_config};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

fn extract_hover_text(hover: tower_lsp::lsp_types::Hover) -> String {
    match hover.contents {
        HoverContents::Scalar(MarkedString::String(text)) => text,
        _ => panic!("Expected scalar string hover contents"),
    }
}

#[test]
fn test_hover_implicit_it_in_collection_method() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CollectionTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun map(transform: (T) -> Int): List<Int> = TODO()
}

class Item(val name: String)

fun test(items: List<Item>) {
    items.map {
        it.name
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(11, 8); // on `it` in `it.name`
    let hover = find_hover(&index, &url, "it", pos, source).expect("hover on it should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: Item"),
        "Hover on it should show 'val it: Item', got:\n{}",
        text
    );
}

#[test]
fn test_hover_implicit_it_in_scope_functions() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ScopeTest.kt").unwrap();
    let source = r#"
package com.example

class User(val name: String)

fun test(user: User) {
    user.let {
        it.name
    }
    user.also {
        val u = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos_let = Position::new(7, 8); // on `it` in `it.name`
    let hover_let = find_hover(&index, &url, "it", pos_let, source)
        .expect("hover on it in let block should resolve");
    let text_let = extract_hover_text(hover_let);
    assert!(
        text_let.contains("val it: User"),
        "Hover in let should show 'val it: User', got:\n{}",
        text_let
    );

    let pos_also = Position::new(10, 16); // on `it` in `val u = it`
    let hover_also = find_hover(&index, &url, "it", pos_also, source)
        .expect("hover on it in also block should resolve");
    let text_also = extract_hover_text(hover_also);
    assert!(
        text_also.contains("val it: User"),
        "Hover in also should show 'val it: User', got:\n{}",
        text_also
    );
}

#[test]
fn test_hover_implicit_it_in_nested_lambdas() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)
class Item(val id: Int)

fun test(users: List<User>, items: List<Item>) {
    users.forEach {
        val u = it
        items.forEach {
            val i = it
        }
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos_u = Position::new(12, 16);
    let hover_u = find_hover(&index, &url, "it", pos_u, source)
        .expect("hover on outer it should resolve to User");
    let text_u = extract_hover_text(hover_u);
    assert!(
        text_u.contains("val it: User"),
        "Outer it should be User, got:\n{}",
        text_u
    );

    let pos_i = Position::new(14, 20);
    let hover_i = find_hover(&index, &url, "it", pos_i, source)
        .expect("hover on inner it should resolve to Item");
    let text_i = extract_hover_text(hover_i);
    assert!(
        text_i.contains("val it: Item"),
        "Inner it should be Item, got:\n{}",
        text_i
    );
}

#[test]
fn test_hover_implicit_it_in_receiver_scope_function_resolves_to_outer_lambda() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/ReceiverScopeTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)
class Config(var timeout: Int)

fun test(users: List<User>, config: Config) {
    users.forEach {
        config.apply {
            val u = it
        }
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(13, 20);
    let hover = find_hover(&index, &url, "it", pos, source)
        .expect("hover on it inside apply inside forEach should resolve to outer User");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: User"),
        "it inside apply should resolve to outer User, got:\n{}",
        text
    );
}

#[test]
fn test_hover_implicit_it_with_always_full_name() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/FullNameTest.kt").unwrap();
    let source = r#"
package com.example.demo

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun test(users: List<User>) {
    users.forEach {
        val u = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(11, 16);
    let hover = find_hover_with_config(
        &index,
        &url,
        "it",
        pos,
        source,
        None,
        VariableHoverTypeInference::AlwaysFullName,
    )
    .expect("hover on it should resolve with full name");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: com.example.demo.User"),
        "AlwaysFullName mode should display fully qualified type, got:\n{}",
        text
    );
}

#[test]
fn test_hover_implicit_it_with_always_trace() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TraceTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)

fun test(users: List<User>) {
    users.forEach {
        val u = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(11, 16);
    let hover = find_hover_with_config(
        &index,
        &url,
        "it",
        pos,
        source,
        None,
        VariableHoverTypeInference::AlwaysWithTrace,
    )
    .expect("hover on it should resolve with trace");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: User"),
        "Hover should show 'val it: User', got:\n{}",
        text
    );
    assert!(
        text.contains("### Type Inference Trace"),
        "Hover should include type inference trace markdown, got:\n{}",
        text
    );
}

#[test]
fn test_hover_implicit_it_in_typed_variable_assignment() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/TypedVarTest.kt").unwrap();
    let source = r#"
package com.example

class User(val name: String)

fun test() {
    val handler: (User) -> Unit = {
        val u = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(7, 16);
    let hover = find_hover(&index, &url, "it", pos, source)
        .expect("hover on it in typed variable lambda should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: User"),
        "Hover in typed variable should show 'val it: User', got:\n{}",
        text
    );
}
