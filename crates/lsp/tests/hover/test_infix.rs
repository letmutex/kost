use lsp::config::VariableHoverTypeInference;
use lsp::features::{find_definition, find_hover, find_hover_with_config};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

fn extract_hover_text(hover: tower_lsp::lsp_types::Hover) -> String {
    match hover.contents {
        HoverContents::Scalar(MarkedString::String(text)) => text,
        _ => panic!("Expected scalar string hover contents"),
    }
}

#[test]
fn test_hover_on_extension_infix_function() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///InfixExt.kt").unwrap();
    let source = r#"
infix fun Int.withUnit(unit: String): String = "$this $unit"

fun test() {
    val str = 1 withUnit "ms"
}
"#;
    index.index_source(url.clone(), source);

    // Hover on `withUnit` in `1 withUnit "ms"` (line 4, character 17)
    let pos = Position::new(4, 17);
    let hover = find_hover(&index, &url, "withUnit", pos, source)
        .expect("hover should resolve for infix function");

    let text = extract_hover_text(hover);
    assert!(
        text.contains("infix fun Int.withUnit(unit: String): String"),
        "Hover text was: {}",
        text
    );

    // Hover on `str` variable with Always type inference
    let var_pos = Position::new(4, 8);
    let var_hover = find_hover_with_config(
        &index,
        &url,
        "str",
        var_pos,
        source,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover should resolve for variable");
    let var_text = extract_hover_text(var_hover);
    assert!(
        var_text.contains("val str: String = 1 withUnit \"ms\""),
        "Variable hover was: {}",
        var_text
    );

    // Go to definition on `withUnit`
    let loc =
        find_definition(&index, &url, "withUnit", pos, source).expect("definition should resolve");
    assert_eq!(loc.range.start.line, 1);
}

#[test]
fn test_hover_on_member_infix_and_single_expression() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Modifier.kt").unwrap();
    let source = r#"
object Dp
object Modifier {
    infix fun then(other: Modifier): Modifier {
        return other
    }
}

fun Modifier.padding(horizontal: Dp = Dp, vertical: Dp = Dp) =
    this then this

fun Modifier.padding(all: Dp) =
    this then this

fun test() {
    val modifier = Modifier.padding(Dp)
}
"#;
    index.index_source(url.clone(), source);

    // Hover on `then` in `this then this` (line 9, character 9)
    let then_pos = Position::new(9, 9);
    let hover = find_hover(&index, &url, "then", then_pos, source)
        .expect("hover should resolve for member infix function");

    let text = extract_hover_text(hover);
    assert!(
        text.contains("infix fun then(other: Modifier): Modifier")
            || text.contains("infix fun Modifier.then(other: Modifier): Modifier"),
        "Hover text was: {}",
        text
    );

    // Go to definition on `then`
    let loc =
        find_definition(&index, &url, "then", then_pos, source).expect("definition should resolve");
    assert_eq!(loc.range.start.line, 3);

    // Hover on `modifier` variable with Always type inference
    let mod_pos = Position::new(15, 8);
    let mod_hover = find_hover_with_config(
        &index,
        &url,
        "modifier",
        mod_pos,
        source,
        None,
        VariableHoverTypeInference::Always,
    )
    .expect("hover should resolve for modifier variable");
    let mod_text = extract_hover_text(mod_hover);
    assert!(
        mod_text.contains("val modifier: Modifier = Modifier.padding(Dp)"),
        "Variable hover was: {}",
        mod_text
    );
}
