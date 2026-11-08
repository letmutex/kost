use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_actual_val_property_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/Timeout.kt").unwrap();
    let common_source = r#"package com.example

/**
 * The default network connection timeout in milliseconds.
 */
expect val defaultNetworkTimeoutMs: Long
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/Timeout.kt").unwrap();
    let actual_source = r#"package com.example

actual val defaultNetworkTimeoutMs: Long = 30000L

fun testUsage() {
    val timeout = defaultNetworkTimeoutMs
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 11);
    let decl_hover = find_hover(
        &index,
        &actual_url,
        "defaultNetworkTimeoutMs",
        decl_pos,
        actual_source,
    )
    .expect("hover on actual val property declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("The default network connection timeout in milliseconds."),
            "Hover on actual val property declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(5, 18);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "defaultNetworkTimeoutMs",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual val property usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("The default network connection timeout in milliseconds."),
            "Hover on actual val property usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_actual_var_property_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/Logging.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Global logging verbosity level.
 */
expect var globalLogLevel: Int
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/Logging.kt").unwrap();
    let actual_source = r#"package com.example

actual var globalLogLevel: Int = 1

fun testUsage() {
    val level = globalLogLevel
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 11);
    let decl_hover = find_hover(
        &index,
        &actual_url,
        "globalLogLevel",
        decl_pos,
        actual_source,
    )
    .expect("hover on actual var property declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Global logging verbosity level."),
            "Hover on actual var property declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(5, 16);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "globalLogLevel",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual var property usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Global logging verbosity level."),
            "Hover on actual var property usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}
