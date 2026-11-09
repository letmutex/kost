use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn hover_actual_class_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/Socket.kt").unwrap();
    let common_source = r#"package com.example

/**
 * Low-level socket handle for network communication across platforms.
 */
expect class SocketConnection
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/Socket.kt").unwrap();
    let actual_source = r#"package com.example

actual class SocketConnection {
    fun close() {}
}

fun testUsage() {
    val connection = SocketConnection()
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(2, 13);
    let decl_hover = find_hover(
        &index,
        &actual_url,
        "SocketConnection",
        decl_pos,
        actual_source,
    )
    .expect("hover on actual class declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Low-level socket handle for network communication across platforms."),
            "Hover on actual class declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 21);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "SocketConnection",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual class constructor call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Low-level socket handle for network communication across platforms."),
            "Hover on actual class constructor call should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_member_function_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/DataBuffer.kt").unwrap();
    let common_source = r#"package com.example

expect class DataBuffer {
    /**
     * Flushes all pending bytes to the underlying sink.
     *
     * @return true if all bytes were successfully flushed.
     */
    fun flush(): Boolean
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/DataBuffer.kt").unwrap();
    let actual_source = r#"package com.example

actual class DataBuffer {
    actual fun flush(): Boolean = true
}

fun testUsage(buffer: DataBuffer) {
    buffer.flush()
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(3, 15);
    let decl_hover = find_hover(&index, &actual_url, "flush", decl_pos, actual_source)
        .expect("hover on actual member function declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Flushes all pending bytes to the underlying sink."),
            "Hover on actual member function declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 11);
    let usage_hover = find_hover(&index, &actual_url, "flush", usage_pos, actual_source)
        .expect("hover on actual member function call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Flushes all pending bytes to the underlying sink."),
            "Hover on actual member function call site should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_member_property_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/SystemInfo.kt").unwrap();
    let common_source = r#"package com.example

expect class SystemInfo {
    /**
     * Total number of available processor cores on this host.
     */
    val availableCores: Int
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/SystemInfo.kt").unwrap();
    let actual_source = r#"package com.example

actual class SystemInfo {
    actual val availableCores: Int = 4
}

fun testUsage(info: SystemInfo) {
    val cores = info.availableCores
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(3, 15);
    let decl_hover = find_hover(
        &index,
        &actual_url,
        "availableCores",
        decl_pos,
        actual_source,
    )
    .expect("hover on actual member property declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Total number of available processor cores on this host."),
            "Hover on actual member property declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 21);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "availableCores",
        usage_pos,
        actual_source,
    )
    .expect("hover on actual member property usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Total number of available processor cores on this host."),
            "Hover on actual member property usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn hover_actual_member_var_property_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/WorkerThread.kt").unwrap();
    let common_source = r#"package com.example

expect class WorkerThread {
    /**
     * Current execution priority of the worker thread.
     */
    var priority: Int
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/WorkerThread.kt").unwrap();
    let actual_source = r#"package com.example

actual class WorkerThread {
    actual var priority: Int = 5
}

fun testUsage(worker: WorkerThread) {
    val p = worker.priority
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(3, 15);
    let decl_hover = find_hover(&index, &actual_url, "priority", decl_pos, actual_source)
        .expect("hover on actual member var property declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Current execution priority of the worker thread."),
            "Hover on actual member var property declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 19);
    let usage_hover = find_hover(&index, &actual_url, "priority", usage_pos, actual_source)
        .expect("hover on actual member var property usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Current execution priority of the worker thread."),
            "Hover on actual member var property usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}
