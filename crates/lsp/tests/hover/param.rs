use lsp::features::{find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_class_parameter_field() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///ConstructorField.kt").unwrap();
    let source = r#"
package com.example

class Repository(
    /**
     * The database DAO for messages.
     */
    val messageDao: MessageDao
) {
    fun fetch() {
        messageDao.get()
    }
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 10,
        character: 10,
    };
    let hover = find_hover(&index, &url, "messageDao", pos, source).expect("hover should resolve");

    let range = hover.range.expect("hover range should be present");
    assert_eq!(range.start.line, 10);
    assert_eq!(range.end.line, 10);

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("val messageDao: MessageDao"));
        assert!(text.contains("The database DAO for messages."));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_function_parameter() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///Param.kt").unwrap();
    let source = r#"
package com.example

class UserPreferences

fun setup(preferences: UserPreferences) {
    val pref = preferences
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 5,
        character: 16,
    };
    let hover = find_hover(&index, &url, "preferences", pos, source)
        .expect("hover on function parameter should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("val preferences: UserPreferences"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_parameter_with_default_and_kdoc() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///DefaultParam.kt").unwrap();
    let source = r#"
package com.example

val defaultPreferences = UserPreferences()

/**
 * Configures the user repository.
 *
 * @param preferences The user preferences to configure.
 * @param timeout Timeout in milliseconds.
 */
fun setupRepository(
    preferences: UserPreferences = defaultPreferences,
    timeout: Long = 1000L
) {
    val p = preferences
}
"#;

    index.index_source(url.clone(), source);

    let pos = Position {
        line: 15,
        character: 14,
    };
    let hover = find_hover(&index, &url, "preferences", pos, source)
        .expect("hover on parameter with default should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("val preferences: UserPreferences = defaultPreferences"));
        assert!(text.contains("The user preferences to configure."));
        assert!(!text.contains("Configures the user repository."));
        assert!(!text.contains("timeout"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_class_hover_excludes_constructor_param_comments() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///AccountService.kt").unwrap();
    let source = r#"
package com.example

class AccountDao

/**
 * Service for managing customer accounts.
 */
class AccountService(
    // Primary database access object
    val accountDao: AccountDao,
    /** Timeout for connection in ms */
    val timeout: Long = 5000L
)

fun main() {
    val service = AccountService(AccountDao(), 1000L)
}
"#;

    index.index_source(url.clone(), source);

    let pos_class = Position {
        line: 15,
        character: 20,
    };
    let hover_class = find_hover(&index, &url, "AccountService", pos_class, source)
        .expect("hover on class should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_class.contents {
        assert!(text.contains("Service for managing customer accounts."));
        assert!(!text.contains("Primary database access object"));
        assert!(!text.contains("Timeout for connection in ms"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_constructor_param_hover_scoping() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///AccountService2.kt").unwrap();
    let source = r#"
package com.example

class AccountDao

/**
 * Service for managing customer accounts.
 */
class AccountService(
    // Primary database access object
    val accountDao: AccountDao,
    /** Timeout for connection in ms */
    val timeout: Long = 5000L
) {
    fun run() {
        val dao = accountDao
        val t = timeout
    }
}
"#;

    index.index_source(url.clone(), source);

    // Hover on 'accountDao' (line 14, col 14)
    let pos_field1 = Position {
        line: 14,
        character: 14,
    };
    let hover_field1 = find_hover(&index, &url, "accountDao", pos_field1, source)
        .expect("hover on accountDao should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_field1.contents {
        assert!(text.contains("val accountDao: AccountDao"));
        assert!(text.contains("Primary database access object"));
        assert!(!text.contains("Service for managing customer accounts."));
        assert!(!text.contains("Timeout for connection in ms"));
    } else {
        panic!("unexpected hover contents type");
    }

    // Hover on 'timeout' (line 15, col 14)
    let pos_field2 = Position {
        line: 15,
        character: 14,
    };
    let hover_field2 = find_hover(&index, &url, "timeout", pos_field2, source)
        .expect("hover on timeout should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_field2.contents {
        assert!(text.contains("val timeout: Long = 5000L"));
        assert!(text.contains("Timeout for connection in ms"));
        assert!(!text.contains("Service for managing customer accounts."));
        assert!(!text.contains("Primary database access object"));
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_catch_block_parameter_resolution_and_hover() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/CatchBlockTest.kt").unwrap();
    let code = r#"
package com.example.err

fun test() {
    try {
        println("work")
    } catch (e: Exception) {
        val err = e
    }
}
"#;
    index.index_source(url.clone(), code);

    // Line 7, col 18: `val err = e`
    let pos_e = Position::new(7, 18);
    let def = find_definition(&index, &url, "e", pos_e, code);
    assert!(
        def.is_some(),
        "catch parameter 'e' should resolve to definition"
    );

    let hover = find_hover(&index, &url, "e", pos_e, code);
    assert!(hover.is_some(), "catch parameter 'e' should resolve hover");
}

#[test]
fn test_named_argument_call_site_definition_and_hover() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NamedArgTest.kt").unwrap();
    let code = r#"
package com.example.args

/**
 * Connects to a remote server.
 * @param timeout Connection timeout in milliseconds
 */
fun connect(timeout: Int, retries: Int) {}

fun test() {
    connect(timeout = 5000, retries = 3)
}
"#;
    index.index_source(url.clone(), code);

    // Line 10, col 14: `timeout` in `connect(timeout = 5000, ...)`
    let pos_arg = Position::new(10, 14);
    let def = find_definition(&index, &url, "timeout", pos_arg, code);
    assert!(
        def.is_some(),
        "named argument 'timeout' at call site should resolve to function parameter definition"
    );

    let hover = find_hover(&index, &url, "timeout", pos_arg, code);
    assert!(
        hover.is_some(),
        "named argument 'timeout' at call site should resolve to hover documentation"
    );
}
