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

fn assert_hover_contains(
    index: &WorkspaceIndex,
    url: &Url,
    symbol: &str,
    pos: Position,
    source: &str,
    expected: &[&str],
) {
    let hover = find_hover(index, url, symbol, pos, source)
        .unwrap_or_else(|| panic!("hover should resolve for '{}'", symbol));
    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        for exp in expected {
            assert!(
                text.contains(exp),
                "Hover text missing expected fragment '{}'. Full hover:\n{}",
                exp,
                text
            );
        }
    } else {
        panic!("unexpected hover contents type");
    }
}

fn build_kdoc(lines: &[&str]) -> String {
    let mut doc = String::from("/**\n");
    for line in lines {
        if line.is_empty() {
            doc.push_str(" *\n");
        } else {
            doc.push_str(&format!(" * {}\n", line));
        }
    }
    doc.push_str(" */\n");
    doc
}

fn build_generated_kdoc(count: usize, line_prefix: &str) -> String {
    let lines: Vec<String> = (0..count)
        .map(|i| {
            format!(
                "{} {} with extensive contextual documentation.",
                line_prefix, i
            )
        })
        .collect();
    let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    build_kdoc(&line_refs)
}

fn build_generated_line_comments(count: usize, line_prefix: &str) -> String {
    let mut comments = String::new();
    for i in 0..count {
        comments.push_str(&format!(
            "// {} {} with detailed background notes.\n",
            line_prefix, i
        ));
    }
    comments
}

#[test]
fn test_hover_generic_interface_after_imports_with_generated_kdoc() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///GenericFlow.kt").unwrap();

    let doc = build_generated_kdoc(40, "Stream pipeline operation step");
    let source = format!(
        "import kotlinx.coroutines.*\nimport kotlin.sequences.*\n\n{}public interface Flow<out T> {{}}\n\nfun consume(flow: Flow<Int>) {{}}\n",
        doc
    );

    index.index_source(url.clone(), &source);

    let def_pos = find_position_of(&source, "Flow<out T>");
    assert_hover_contains(
        &index,
        &url,
        "Flow",
        def_pos,
        &source,
        &[
            "```kotlin\npublic interface Flow<out T>\n```",
            "Stream pipeline operation step 0",
            "Stream pipeline operation step 39",
        ],
    );

    let usage_pos = find_position_of(&source, "Flow<Int>");
    assert_hover_contains(
        &index,
        &url,
        "Flow",
        usage_pos,
        &source,
        &[
            "```kotlin\npublic interface Flow<out T>\n```",
            "Stream pipeline operation step 0",
            "Stream pipeline operation step 39",
        ],
    );
}

#[test]
fn test_hover_large_kdoc_over_ten_kb() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///LargeDoc.kt").unwrap();

    let doc = build_generated_kdoc(120, "Architecture layer component description");
    let source = format!(
        "package com.example\n\nimport java.util.concurrent.*\n\n{}class LargeService\n\nval serviceInstance = LargeService()\n",
        doc
    );

    index.index_source(url.clone(), &source);

    let def_pos = find_position_of(&source, "LargeService\n");
    assert_hover_contains(
        &index,
        &url,
        "LargeService",
        def_pos,
        &source,
        &[
            "com.example\n\n```kotlin\nclass LargeService\n```",
            "Architecture layer component description 0",
            "Architecture layer component description 119",
        ],
    );

    let usage_pos = find_position_of(&source, "LargeService()");
    assert_hover_contains(
        &index,
        &url,
        "LargeService",
        usage_pos,
        &source,
        &[
            "com.example\n\n```kotlin\nclass LargeService\n```",
            "Architecture layer component description 0",
            "Architecture layer component description 119",
        ],
    );
}

#[test]
fn test_hover_large_line_comments_before_property() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///LargeLineDoc.kt").unwrap();

    let comments = build_generated_line_comments(60, "Config explanation entry");
    let source = format!(
        "import kotlin.math.*\n\n{}val longDocConfig = 42\n\nval readConfig = longDocConfig\n",
        comments
    );

    index.index_source(url.clone(), &source);

    let def_pos = find_position_of(&source, "longDocConfig = 42");
    assert_hover_contains(
        &index,
        &url,
        "longDocConfig",
        def_pos,
        &source,
        &[
            "```kotlin\nval longDocConfig = 42\n```",
            "Config explanation entry 0",
            "Config explanation entry 59",
        ],
    );

    let usage_pos = find_position_of(&source, "longDocConfig\n");
    assert_hover_contains(
        &index,
        &url,
        "longDocConfig",
        usage_pos,
        &source,
        &[
            "```kotlin\nval longDocConfig = 42\n```",
            "Config explanation entry 0",
            "Config explanation entry 59",
        ],
    );
}
