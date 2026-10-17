use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use lsp::syntax::doc::clean_kdoc;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_function_kdoc_code_block_preserves_indentation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ConfigService.kt").unwrap();
    let source = r#"
package com.example

/**
 * Builds a user configuration.
 *
 * Example:
 * ```kotlin
 * val config = buildConfig {
 *     timeout = 30
 *     retries = 3
 *     nested {
 *         enabled = true
 *         level = 2
 *     }
 * }
 * ```
 * @param name configuration name
 * @return the configured Config instance
 */
fun buildConfig(name: String): Config {
    return Config(name)
}

fun main() {
    buildConfig("app")
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position {
        line: 23,
        character: 6,
    };
    let hover = find_hover(&index, &url, "buildConfig", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("com.example\n\n```kotlin\nfun buildConfig(name: String): Config\n```")
        );
        assert!(text.contains("Builds a user configuration."));
        assert!(text.contains("```kotlin\nval config = buildConfig {\n    timeout = 30\n    retries = 3\n    nested {\n        enabled = true\n        level = 2\n    }\n}\n```"));
        assert!(text.contains("* **@param** `name` — configuration name"));
        assert!(text.contains("* **@return** — the configured Config instance"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_class_kdoc_code_block_preserves_indentation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///HttpClient.kt").unwrap();
    let source = r#"
package com.example

/**
 * HTTP client for API requests.
 *
 * ```kotlin
 * val client = HttpClient {
 *     install(Logging) {
 *         level = Level.ALL
 *         logger = Logger.DEFAULT
 *     }
 * }
 * ```
 */
class HttpClient(val config: Config)

fun test() {
    HttpClient(Config("default"))
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position {
        line: 18,
        character: 6,
    };
    let hover = find_hover(&index, &url, "HttpClient", pos, source)
        .expect("hover should resolve on HttpClient");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("com.example\n\n```kotlin\nclass HttpClient(val config: Config)\n```")
        );
        assert!(text.contains("```kotlin\nval client = HttpClient {\n    install(Logging) {\n        level = Level.ALL\n        logger = Logger.DEFAULT\n    }\n}\n```"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_property_line_comment_code_block_indentation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Settings.kt").unwrap();
    let source = r#"
package com.example

// Application settings holder.
//
// Usage:
// ```kotlin
// if (settings.isDebug) {
//     enableDebugMode()
// }
// ```
val isDebug: Boolean = true

fun check() {
    val d = isDebug
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position {
        line: 14,
        character: 13,
    };
    let hover =
        find_hover(&index, &url, "isDebug", pos, source).expect("hover should resolve on isDebug");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("```kotlin\nif (settings.isDebug) {\n    enableDebugMode()\n}\n```"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_param_kdoc_code_block_indentation() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Api.kt").unwrap();
    let source = r#"
package com.example

/**
 * Configures the service.
 * @param handler lambda handler:
 * ```kotlin
 * handler = { event ->
 *     when (event) {
 *         is Event.Start -> start()
 *         is Event.Stop -> stop()
 *     }
 * }
 * ```
 */
fun configure(handler: (Event) -> Unit) {}

fun call() {
    configure(handler = {})
}
"#;
    index.index_source(url.clone(), source);

    let pos = Position {
        line: 18,
        character: 16,
    };
    let hover = find_hover(&index, &url, "handler", pos, source)
        .expect("hover should resolve on parameter handler");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("```kotlin\nhandler = { event ->\n    when (event) {\n        is Event.Start -> start()\n        is Event.Stop -> stop()\n    }\n}\n```"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_clean_kdoc_preserves_code_block_indentation_direct() {
    let raw_kdoc = r#"
/**
 * Processes items with custom strategy.
 *
 * ```kotlin
 * fun execute() {
 *     val item = Item(1)
 *     if (item.isValid) {
 *         process(item)
 *     }
 * }
 * ```
 *
 * Also supports nested blocks:
 * ~~~kotlin
 * val result = run {
 *     compute {
 *         step()
 *     }
 * }
 * ~~~
 *
 * @param strategy the execution strategy
 * @return the final result
 */
"#;

    let cleaned = clean_kdoc(raw_kdoc);
    assert!(cleaned.contains("Processes items with custom strategy."));
    assert!(cleaned.contains("```kotlin\nfun execute() {\n    val item = Item(1)\n    if (item.isValid) {\n        process(item)\n    }\n}\n```"));
    assert!(
        cleaned.contains(
            "~~~kotlin\nval result = run {\n    compute {\n        step()\n    }\n}\n~~~"
        )
    );
    assert!(cleaned.contains("* **@param** `strategy` — the execution strategy"));
    assert!(cleaned.contains("* **@return** — the final result"));
}

#[test]
fn test_clean_kdoc_tag_inside_code_block_not_converted() {
    let raw = r#"
/**
 * Example demonstrating annotation usage:
 * ```kotlin
 * @param name
 * fun example()
 * ```
 */
"#;
    let cleaned = clean_kdoc(raw);
    assert!(cleaned.contains("```kotlin\n@param name\nfun example()\n```"));
    assert!(!cleaned.contains("* **@param**"));
}
