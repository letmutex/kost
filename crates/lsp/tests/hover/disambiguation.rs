use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_disambiguates_same_named_function_and_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/SameNameTest.kt").unwrap();
    let code = "fun fib(x: Int) {}\nval fib = 10\nval x = fib + 10\nfun call() {\n    fib(5)\n}";
    index.index_source(url.clone(), code);

    // Function call `fib(5)` at line 4, col 4 should resolve to `fun fib(x: Int)`
    let pos_call = Position::new(4, 4);
    let hover_fn = find_hover(&index, &url, "fib", pos_call, code)
        .expect("hover on function call fib(5) should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_fn.contents {
        assert!(
            text.contains("fun fib"),
            "Hover on fib(5) should show 'fun fib'"
        );
    }

    // Property expression `fib + 10` at line 2, col 8 should resolve to `val fib`
    let pos_prop = Position::new(2, 8);
    let hover_prop = find_hover(&index, &url, "fib", pos_prop, code)
        .expect("hover on property expression fib + 10 should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_prop.contents {
        assert!(
            text.contains("val fib"),
            "Hover on 'fib' in 'fib + 10' should show property 'val fib', not function"
        );
        assert!(
            !text.contains("fun fib"),
            "Hover on 'fib' in 'fib + 10' should not resolve to 'fun fib'"
        );
    }
}

#[test]
fn test_hover_disambiguates_extension_function_and_lambda_parameter() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/Modifier.kt").unwrap();
    let file_a_code = r#"package example.base

public interface Modifier2 {
    companion object : Modifier2 {}
}

fun Modifier2.padding(value: Int): Modifer2 {
    return this
}
"#;

    let file_b_url = Url::parse("file:///project/Component.kt").unwrap();
    let file_b_code = r#"import example.base.Modifier2
import example.base.padding

fun Component(content: (padding: Int) -> Unit) {}

fun main() {
    val modifier = Modifier2
    Component { padding ->
        modifier.padding(padding)
    }
}
"#;

    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    // Lambda parameter declaration: `padding ->` (line 7, col 16)
    let pos_lambda_param = Position::new(7, 16);
    let hover_lambda_param = find_hover(
        &index,
        &file_b_url,
        "padding",
        pos_lambda_param,
        file_b_code,
    )
    .expect("hover on lambda parameter declaration `padding ->` should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_lambda_param.contents {
        assert!(
            text.contains("val padding"),
            "Hover on lambda parameter declaration should show 'val padding': {}",
            text
        );
    }

    // Extension function call: `modifier.padding(` (line 8, col 17)
    let pos_fn_call = Position::new(8, 17);
    let hover_fn_call = find_hover(&index, &file_b_url, "padding", pos_fn_call, file_b_code)
        .expect("hover on extension function call `modifier.padding` should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_fn_call.contents {
        assert!(
            text.contains("fun Modifier2.padding(value: Int): Modifer2"),
            "Hover on modifier.padding should show extension function signature: {}",
            text
        );
        assert!(
            text.contains("Modifier.kt"),
            "Hover on modifier.padding should reference Modifier.kt: {}",
            text
        );
    }

    // Lambda parameter argument usage: `(padding)` (line 8, col 25)
    let pos_param_arg = Position::new(8, 25);
    let hover_param_arg = find_hover(&index, &file_b_url, "padding", pos_param_arg, file_b_code)
        .expect("hover on lambda parameter argument `(padding)` should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_param_arg.contents {
        assert!(
            text.contains("val padding"),
            "Hover on lambda parameter argument should show 'val padding': {}",
            text
        );
    }
}
