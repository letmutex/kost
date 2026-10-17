use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use lsp::syntax::doc::clean_kdoc;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_clean_kdoc_renders_code_links_to_inline_code() {
    let raw = r#"
/**
 * Remembers the value produced by [calculation].
 *
 * Use [remember] with [mutableStateOf] to create a [MutableState].
 * See [androidx.compose.runtime.remember] for details.
 *
 * Reference link: [the factory method][remember]
 *
 * Markdown link preserved: [Kotlin Docs](https://kotlinlang.org)
 * Image preserved: ![architecture](https://example.com/arch.png)
 *
 * Checkboxes preserved:
 * - [ ] todo item
 * - [x] done item
 *
 * Code block preserved:
 * ```kotlin
 * val x = array[0]
 * val y = map["key"]
 * ```
 *
 * @param calculation the [Function0] value producer
 * @return the [T] remembered value
 * @throws [IllegalStateException] if called outside of [Composition]
 * @see [rememberSaveable]
 */
"#;
    let cleaned = clean_kdoc(raw);

    assert!(cleaned.contains("Remembers the value produced by `calculation`."));
    assert!(cleaned.contains("Use `remember` with `mutableStateOf` to create a `MutableState`."));
    assert!(cleaned.contains("See `androidx.compose.runtime.remember` for details."));
    assert!(cleaned.contains("Reference link: `the factory method`"));
    assert!(cleaned.contains("Markdown link preserved: [Kotlin Docs](https://kotlinlang.org)"));
    assert!(cleaned.contains("Image preserved: ![architecture](https://example.com/arch.png)"));
    assert!(cleaned.contains("- [ ] todo item"));
    assert!(cleaned.contains("- [x] done item"));
    assert!(cleaned.contains("```kotlin\nval x = array[0]\nval y = map[\"key\"]\n```"));
    assert!(cleaned.contains("* **@param** `calculation` — the `Function0` value producer"));
    assert!(cleaned.contains("* **@return** — the `T` remembered value"));
    assert!(
        cleaned
            .contains("* **@throws** `IllegalStateException` — if called outside of `Composition`")
    );
    assert!(cleaned.contains("* **@see** `rememberSaveable`"));
}

#[test]
fn test_hover_renders_code_links_for_function_and_parameters() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///State.kt").unwrap();
    let source = r#"
package com.example

/**
 * Creates and remembers a [State] instance with [initialValue].
 *
 * Refer to [remember] and [MutableState] for details.
 *
 * @param initialValue the [T] initial state value
 * @return a remembered [State] wrapping [initialValue]
 */
fun <T> rememberState(initialValue: T): State<T> {
    return State(initialValue)
}

fun usage() {
    rememberState(42)
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position {
        line: 14,
        character: 6,
    };
    let hover = find_hover(&index, &url, "rememberState", pos, source)
        .expect("hover should resolve on rememberState");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("Creates and remembers a `State` instance with `initialValue`."));
        assert!(text.contains("Refer to `remember` and `MutableState` for details."));
        assert!(text.contains("* **@param** `initialValue` — the `T` initial state value"));
        assert!(text.contains("* **@return** — a remembered `State` wrapping `initialValue`"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_renders_code_links_for_line_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Util.kt").unwrap();
    let source = r#"
package com.example

// Wraps [item] into a [List] with [size] elements.
// See [Collections.singletonList] for reference.
val sampleList: List<String> = listOf("item")

fun check() {
    val x = sampleList
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position {
        line: 7,
        character: 13,
    };
    let hover = find_hover(&index, &url, "sampleList", pos, source)
        .expect("hover should resolve on sampleList");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("Wraps `item` into a `List` with `size` elements."));
        assert!(text.contains("See `Collections.singletonList` for reference."));
    } else {
        panic!("unexpected hover contents type");
    }
}
