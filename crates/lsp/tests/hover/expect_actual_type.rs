use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn hover_actual_interface_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/AudioSink.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Platform audio sink interface.
 */
expect interface AudioSink {
    fun renderAudio()
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/AudioSink.kt").unwrap();
    let actual_source = r#"package com.example

actual interface AudioSink {
    actual fun renderAudio()
}

fun playAudio(sink: AudioSink) {
    sink.renderAudio()
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 17);
    let decl_hover = find_hover(&index, &actual_url, "AudioSink", decl_pos, actual_source)
        .expect("hover on actual interface declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Platform audio sink interface."),
            "Hover on actual interface declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(6, 20);
    let usage_hover = find_hover(&index, &actual_url, "AudioSink", usage_pos, actual_source)
        .expect("hover on actual interface type reference should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Platform audio sink interface."),
            "Hover on actual interface type reference should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_object_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/CryptoEngine.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Singleton providing access to the platform cryptographic subsystem.
 */
expect object CryptoEngine {
    fun generateSeed(): ByteArray
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/CryptoEngine.kt").unwrap();
    let actual_source = r#"package com.example

actual object CryptoEngine {
    actual fun generateSeed(): ByteArray = ByteArray(32)
}

fun testUsage() {
    val engine = CryptoEngine
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 14);
    let decl_hover = find_hover(&index, &actual_url, "CryptoEngine", decl_pos, actual_source)
        .expect("hover on actual object declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Singleton providing access to the platform cryptographic subsystem."),
            "Hover on actual object declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 17);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "CryptoEngine",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual object usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Singleton providing access to the platform cryptographic subsystem."),
            "Hover on actual object usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_companion_object_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/ClientEndpoint.kt").unwrap();
    let common_source = r#"package com.example

expect class ClientEndpoint {
    /**
     * Factory methods for creating ClientEndpoint instances.
     */
    companion object Factory
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/ClientEndpoint.kt").unwrap();
    let actual_source = r#"package com.example

actual class ClientEndpoint {
    actual companion object Factory
}

fun testUsage() {
    val factory = ClientEndpoint.Factory
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(3, 28);
    let decl_hover = find_hover(&index, &actual_url, "Factory", decl_pos, actual_source)
        .expect("hover on actual companion object declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Factory methods for creating ClientEndpoint instances."),
            "Hover on actual companion object declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 33);
    let usage_hover = find_hover(&index, &actual_url, "Factory", usage_pos, actual_source)
        .expect("hover on actual companion object usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Factory methods for creating ClientEndpoint instances."),
            "Hover on actual companion object usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_typealias_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/PlatformFile.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Platform filesystem path representation.
 */
expect class PlatformFile
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/PlatformFile.kt").unwrap();
    let actual_source = r#"package com.example

actual typealias PlatformFile = String

fun testUsage(file: PlatformFile) {}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 17);
    let decl_hover = find_hover(&index, &actual_url, "PlatformFile", decl_pos, actual_source)
        .expect("hover on actual typealias declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Platform filesystem path representation."),
            "Hover on actual typealias declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(4, 20);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "PlatformFile",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual typealias type reference should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Platform filesystem path representation."),
            "Hover on actual typealias type reference should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}
