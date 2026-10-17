use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_multiline_class_constructor_and_annotations() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///MessageRepository.kt").unwrap();
    let source = r#"
package com.example

/**
 * Repository responsible for managing chat messages.
 * @param messageDao the message DAO instance
 * @param dispatcher coroutine dispatcher for IO operations
 * @see MessageDao
 */
@Serializable
data class MessageRepository(
    val messageDao: MessageDao,
    val dispatcher: CoroutineDispatcher = Dispatchers.IO
) : Repository {
    fun getAll() = emptyList<String>()
}

fun usage() {
    val repo = MessageRepository(MessageDao())
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 18,
        character: 18,
    };
    let hover =
        find_hover(&index, &url, "MessageRepository", pos, source).expect("hover should resolve");

    let range = hover.range.expect("hover range should be present");
    assert_eq!(range.start.line, 18);
    assert_eq!(range.end.line, 18);

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("@Serializable"));
        assert!(text.contains("data class MessageRepository("));
        assert!(text.contains("val messageDao: MessageDao,"));
        assert!(text.contains("val dispatcher: CoroutineDispatcher = Dispatchers.IO"));
        assert!(text.contains(") : Repository"));
        assert!(text.contains("Repository responsible for managing chat messages."));
        assert!(text.contains("* **@param** `messageDao` — the message DAO instance"));
        assert!(
            text.contains("* **@param** `dispatcher` — coroutine dispatcher for IO operations")
        );
        assert!(text.contains("* **@see** `MessageDao`"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_typealias_object_and_enum() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Types.kt").unwrap();
    let source = r#"
package com.example

/**
 * Mapping of chat id to list of messages.
 */
typealias MessageMap = Map<String, List<Message>>

/**
 * Global application configuration.
 */
object AppConfig {
    const val VERSION = "1.0.0"
}

enum class Status {
    ACTIVE,
    INACTIVE
}

fun test() {
    val m: MessageMap = emptyMap()
    val cfg = AppConfig
    val s = Status.ACTIVE
}
"#;

    index.index_source(url.clone(), source);

    // 1. TypeAlias hover
    let pos_alias = Position {
        line: 21,
        character: 14,
    };
    let hover_alias =
        find_hover(&index, &url, "MessageMap", pos_alias, source).expect("hover should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_alias.contents {
        assert!(text.contains("typealias MessageMap = Map<String, List<Message>>"));
        assert!(text.contains("Mapping of chat id to list of messages."));
    }

    // 2. Object hover
    let pos_obj = Position {
        line: 22,
        character: 16,
    };
    let hover_obj =
        find_hover(&index, &url, "AppConfig", pos_obj, source).expect("hover should resolve");
    if let HoverContents::Scalar(MarkedString::String(text)) = hover_obj.contents {
        assert!(text.contains("object AppConfig"));
        assert!(text.contains("Global application configuration."));
    }
}

#[test]
fn test_hover_class_strips_inline_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///StateHolder.kt").unwrap();
    let source = r#"
package com.example

// Global application state holder
@Serializable
class StateHolder(
    // Database access DAO
    val messageDao: MessageDao,
    /* Coroutine IO dispatcher */
    val dispatcher: CoroutineDispatcher = Dispatchers.IO
)

fun test() {
    val s = StateHolder(MessageDao())
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 13,
        character: 14,
    };
    let hover = find_hover(&index, &url, "StateHolder", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let parts: Vec<&str> = text.split("```").collect();
        assert!(parts.len() >= 3);
        let code_block = parts[1];
        assert!(!code_block.contains("// Database access DAO"));
        assert!(!code_block.contains("/* Coroutine IO dispatcher */"));
        assert!(code_block.contains("@Serializable"));
        assert!(code_block.contains("class StateHolder("));
        assert!(code_block.contains("val messageDao: MessageDao,"));
        assert!(code_block.contains("val dispatcher: CoroutineDispatcher = Dispatchers.IO"));

        assert!(text.contains("Global application state holder"));
        assert!(!text.contains("Database access DAO"));
        assert!(!text.contains("Coroutine IO dispatcher"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_fields_retain_own_inline_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///StateHolder2.kt").unwrap();
    let source = r#"
package com.example

// Global application state holder
@Serializable
class StateHolder(
    // Database access DAO
    val messageDao: MessageDao,
    /* Coroutine IO dispatcher */
    val dispatcher: CoroutineDispatcher = Dispatchers.IO
)
"#;

    index.index_source(url.clone(), source);

    // 1. Hover on 'messageDao' (line 7, col 8)
    let pos_dao = Position {
        line: 7,
        character: 8,
    };
    let hover_dao = find_hover(&index, &url, "messageDao", pos_dao, source)
        .expect("hover on messageDao should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_dao.contents {
        assert!(text.contains("val messageDao: MessageDao"));
        assert!(text.contains("Database access DAO"));
        assert!(!text.contains("Global application state holder"));
    } else {
        panic!("unexpected hover contents type");
    }

    // 2. Hover on 'dispatcher' (line 9, col 8)
    let pos_disp = Position {
        line: 9,
        character: 8,
    };
    let hover_disp = find_hover(&index, &url, "dispatcher", pos_disp, source)
        .expect("hover on dispatcher should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_disp.contents {
        assert!(text.contains("val dispatcher: CoroutineDispatcher = Dispatchers.IO"));
        assert!(text.contains("Coroutine IO dispatcher"));
        assert!(!text.contains("Global application state holder"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_annotations_on_top_of_kdoc_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///AnnotatedDoc.kt").unwrap();
    let source = r#"
package com.example

@Serializable
/**
 * The comment
 */
data class User(
    val id: String
)

fun test() {
    val u = User("1")
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 12,
        character: 13,
    };
    let hover = find_hover(&index, &url, "User", pos, source).expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        let parts: Vec<&str> = text.split("```").collect();
        assert!(parts.len() >= 3);
        let code_block = parts[1];
        assert!(!code_block.contains("/**"));
        assert!(!code_block.contains("The comment"));
        assert!(code_block.contains("@Serializable"));
        assert!(code_block.contains("data class User("));
        assert!(code_block.contains("val id: String"));

        assert!(text.contains("The comment"));
        assert!(!text.contains("/**"));
    } else {
        panic!("unexpected hover contents type");
    }
}
