use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

fn find_position_of(source: &str, pattern: &str) -> Position {
    let offset = source
        .find(pattern)
        .unwrap_or_else(|| panic!("pattern '{}' not found in source", pattern));
    let mut line = 0;
    let mut character = 0;
    for (i, ch) in source.char_indices() {
        if i == offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Position { line, character }
}

#[test]
fn test_hover_delegated_var_state_by_retain() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///StateHolder.kt").unwrap();
    let source = r#"package com.example

fun render() {
    var state by retain { mutableStateOf(0) }
    val current = state
}
"#;
    index.index_source(url.clone(), source);

    let def_pos = find_position_of(source, "state by retain");
    let hover_def = find_hover(&index, &url, "state", def_pos, source)
        .expect("hover should resolve on delegated var state definition");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_def.contents {
        assert!(
            text.contains("var state by retain { mutableStateOf(0) }"),
            "Hover on delegated var definition should include full delegate signature, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = find_position_of(source, "state\n");
    let hover_usage = find_hover(&index, &url, "state", usage_pos, source)
        .expect("hover should resolve on delegated var state usage");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_usage.contents {
        assert!(
            text.contains("var state by retain { mutableStateOf(0) }"),
            "Hover on delegated var usage should include full delegate signature, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_delegated_var_selected_tab_by_remember() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///TabComponent.kt").unwrap();
    let source = r#"package com.example

fun tabContainer() {
    var selectedTab by remember { mutableStateOf(0) }
    val active = selectedTab
}
"#;
    index.index_source(url.clone(), source);

    let def_pos = find_position_of(source, "selectedTab by remember");
    let hover_def = find_hover(&index, &url, "selectedTab", def_pos, source)
        .expect("hover should resolve on delegated var selectedTab definition");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_def.contents {
        assert!(
            text.contains("var selectedTab by remember { mutableStateOf(0) }"),
            "Hover on delegated var definition should include full delegate signature, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = find_position_of(source, "selectedTab\n");
    let hover_usage = find_hover(&index, &url, "selectedTab", usage_pos, source)
        .expect("hover should resolve on delegated var selectedTab usage");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_usage.contents {
        assert!(
            text.contains("var selectedTab by remember { mutableStateOf(0) }"),
            "Hover on delegated var usage should include full delegate signature, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}
