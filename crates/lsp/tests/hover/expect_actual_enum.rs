use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_actual_enum_class_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/Architecture.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Target CPU architecture representation.
 */
expect enum class Architecture {
    X86_64,
    ARM64,
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/Architecture.kt").unwrap();
    let actual_source = r#"package com.example

actual enum class Architecture {
    X86_64,
    ARM64,
}

fun testUsage() {
    val arch = Architecture.ARM64
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 18);
    let decl_hover = find_hover(&index, &actual_url, "Architecture", decl_pos, actual_source)
        .expect("hover on actual enum class declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Target CPU architecture representation."),
            "Hover on actual enum class declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(8, 15);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "Architecture",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual enum class reference should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Target CPU architecture representation."),
            "Hover on actual enum class reference should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_actual_enum_entry_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/LogSeverity.kt").unwrap();
    let common_source = r#"package com.example

expect enum class LogSeverity {
    /**
     * Detailed diagnostic information for debugging.
     */
    DEBUG,

    /**
     * Critical failure events causing termination.
     */
    FATAL,
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/LogSeverity.kt").unwrap();
    let actual_source = r#"package com.example

actual enum class LogSeverity {
    DEBUG,
    FATAL,
}

fun testUsage() {
    val level = LogSeverity.DEBUG
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(3, 4);
    let decl_hover = find_hover(&index, &actual_url, "DEBUG", decl_pos, actual_source)
        .expect("hover on actual enum entry declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Detailed diagnostic information for debugging."),
            "Hover on actual enum entry declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(8, 28);
    let usage_hover = find_hover(&index, &actual_url, "DEBUG", usage_pos, actual_source)
        .expect("hover on actual enum entry usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Detailed diagnostic information for debugging."),
            "Hover on actual enum entry usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_actual_annotation_class_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/MainThread.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Designates a declaration that is restricted to the main UI thread.
 */
expect annotation class MainThread()
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/MainThread.kt").unwrap();
    let actual_source = r#"package com.example

actual annotation class MainThread()

@MainThread
fun updateUi() {}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 24);
    let decl_hover = find_hover(&index, &actual_url, "MainThread", decl_pos, actual_source)
        .expect("hover on actual annotation class declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Designates a declaration that is restricted to the main UI thread."),
            "Hover on actual annotation class declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(4, 1);
    let usage_hover = find_hover(&index, &actual_url, "MainThread", usage_pos, actual_source)
        .expect("hover on actual annotation usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Designates a declaration that is restricted to the main UI thread."),
            "Hover on actual annotation usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}
