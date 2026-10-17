use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

fn extract_hover_text(hover: tower_lsp::lsp_types::Hover) -> String {
    match hover.contents {
        HoverContents::Scalar(MarkedString::String(text)) => text,
        _ => panic!("Expected scalar string hover contents"),
    }
}

#[test]
fn test_implicit_it_inner_explicit_lambda_resolves_to_outer_lambda() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedExplicitTest.kt").unwrap();
    let source = r#"
package com.example

class List<T> {
    fun forEach(action: (T) -> Unit) {}
}

class User(val name: String)
class Item(val id: Int)

fun test(users: List<User>, items: List<Item>) {
    users.forEach {
        items.forEach { item ->
            val u = it
        }
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(13, 20); // on `it` in `val u = it`
    let hover = find_hover(&index, &url, "it", pos, source)
        .expect("hover on it inside inner explicit lambda should resolve to outer lambda's User");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: User"),
        "Hover inside explicit-param inner lambda should resolve to outer implicit 'val it: User', got:\n{}",
        text
    );
}

#[test]
fn test_implicit_it_in_nullable_functional_type_declaration() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NullableFuncDeclTest.kt").unwrap();
    let source = r#"
package com.example

class User(val name: String)

fun test() {
    val handler: ((User) -> Unit)? = {
        val u = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(7, 16);
    let hover = find_hover(&index, &url, "it", pos, source)
        .expect("hover on it in nullable functional type assignment should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: User"),
        "Hover in nullable functional type assignment should show 'val it: User', got:\n{}",
        text
    );
}

#[test]
fn test_implicit_it_in_nullable_functional_parameter_call() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NullableFuncParamTest.kt").unwrap();
    let source = r#"
package com.example

class User(val name: String)

fun register(callback: ((User) -> Unit)?) {}

fun test() {
    register {
        val u = it
    }
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position::new(9, 16);
    let hover = find_hover(&index, &url, "it", pos, source)
        .expect("hover on it in nullable functional parameter call should resolve");
    let text = extract_hover_text(hover);
    assert!(
        text.contains("val it: User"),
        "Hover in nullable functional parameter call should show 'val it: User', got:\n{}",
        text
    );
}
