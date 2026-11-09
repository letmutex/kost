use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn hover_actual_function_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/Platform.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Returns the current platform operating system name.
 *
 * @return the platform OS name.
 */
expect fun currentPlatform(): String
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/Platform.kt").unwrap();
    let actual_source = r#"package com.example

actual fun currentPlatform(): String = "Linux"

fun testUsage() {
    val platform = currentPlatform()
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 11);
    let decl_hover = find_hover(
        &index,
        &actual_url,
        "currentPlatform",
        decl_pos,
        actual_source,
    )
    .expect("hover on actual function declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Returns the current platform operating system name."),
            "Hover on actual function declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let call_pos = Position::new(5, 19);
    let call_hover = find_hover(
        &index,
        &actual_url,
        "currentPlatform",
        call_pos,
        actual_source,
    )
    .expect("hover on actual function call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = call_hover.contents {
        assert!(
            text.contains("Returns the current platform operating system name."),
            "Hover on actual function call site should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_overloaded_function_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/MessageSender.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Sends a UTF-8 text message over the channel.
 */
expect fun sendMessage(payload: String)

/**
 * Sends raw binary data with route header over the channel.
 */
expect fun sendMessage(payload: ByteArray, route: String)
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/MessageSender.kt").unwrap();
    let actual_source = r#"package com.example

actual fun sendMessage(payload: String) {}

actual fun sendMessage(payload: ByteArray, route: String) {}

fun testUsage(bytes: ByteArray) {
    sendMessage("hello")
    sendMessage(bytes, "primary")
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let text_call_pos = Position::new(7, 4);
    let text_hover = find_hover(
        &index,
        &actual_url,
        "sendMessage",
        text_call_pos,
        actual_source,
    )
    .expect("hover on text sendMessage call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = text_hover.contents {
        assert!(
            text.contains("Sends a UTF-8 text message over the channel."),
            "Hover on text sendMessage call should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let binary_call_pos = Position::new(8, 4);
    let binary_hover = find_hover(
        &index,
        &actual_url,
        "sendMessage",
        binary_call_pos,
        actual_source,
    )
    .expect("hover on binary sendMessage call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = binary_hover.contents {
        assert!(
            text.contains("Sends raw binary data with route header over the channel."),
            "Hover on binary sendMessage call should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_extension_function_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/Hex.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Converts a byte array to a hex-encoded string.
 */
expect fun ByteArray.toHexString(): String
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/Hex.kt").unwrap();
    let actual_source = r#"package com.example

actual fun ByteArray.toHexString(): String = ""

fun testUsage(bytes: ByteArray) {
    val hex = bytes.toHexString()
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let call_pos = Position::new(5, 20);
    let hover = find_hover(&index, &actual_url, "toHexString", call_pos, actual_source)
        .expect("hover on extension function call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("Converts a byte array to a hex-encoded string."),
            "Hover on actual extension function call site should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}
