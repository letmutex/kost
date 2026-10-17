use lsp::features::{find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_and_definition_do_not_leak_unrelated_class_members_without_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UserScopeHoverTest.kt").unwrap();
    let code = r#"
package com.example.model

class User(val id: String, val name: String) {
    val isActive: Boolean = true
    fun getDisplayName(): String = name
}

fun outsideFunction() {
    getDisplayName
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 8);
    let def = find_definition(&index, &url, "getDisplayName", pos, code);
    let hover = find_hover(&index, &url, "getDisplayName", pos, code);

    assert!(
        def.is_none(),
        "Definition of User.getDisplayName must NOT resolve for bare identifier outside class"
    );
    assert!(
        hover.is_none(),
        "Hover of User.getDisplayName must NOT resolve for bare identifier outside class"
    );
}

#[test]
fn test_hover_and_definition_resolve_class_members_inside_class() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UserScopeInsideTest.kt").unwrap();
    let code = r#"
package com.example.model

class User(val id: String, val name: String) {
    val isActive: Boolean = true
    fun getDisplayName(): String = name

    fun validate() {
        getDisplayName
    }
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(8, 12);
    let def = find_definition(&index, &url, "getDisplayName", pos, code);
    let hover = find_hover(&index, &url, "getDisplayName", pos, code);

    assert!(
        def.is_some(),
        "Definition of getDisplayName should resolve inside class"
    );
    assert!(
        hover.is_some(),
        "Hover of getDisplayName should resolve inside class"
    );
    if let HoverContents::Scalar(MarkedString::String(text)) = hover.unwrap().contents {
        assert!(
            text.contains("getDisplayName"),
            "Hover should show getDisplayName signature"
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_and_definition_resolve_with_receiver() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/UserReceiverHoverTest.kt").unwrap();
    let code = r#"
package com.example.model

class User(val id: String, val name: String) {
    fun getDisplayName(): String = name
}

fun outsideFunction() {
    val user = User("1", "Alice")
    user.getDisplayName()
}
"#;
    index.index_source(url.clone(), code);

    let pos = Position::new(9, 12);
    let def = find_definition(&index, &url, "getDisplayName", pos, code);
    let hover = find_hover(&index, &url, "getDisplayName", pos, code);

    assert!(
        def.is_some(),
        "Definition should resolve for user.getDisplayName()"
    );
    assert!(
        hover.is_some(),
        "Hover should resolve for user.getDisplayName()"
    );
}

#[test]
fn test_hover_and_definition_nested_and_companion_scoping() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/NestedCompanionHoverTest.kt").unwrap();
    let code = r#"
package com.example.scoped

class Config {
    companion object {
        val DEFAULT_TIMEOUT: Int = 5000
    }

    fun load() {
        DEFAULT_TIMEOUT
    }
}

fun outside() {
    DEFAULT_TIMEOUT
}
"#;
    index.index_source(url.clone(), code);

    let inside_pos = Position::new(9, 10);
    assert!(
        find_hover(&index, &url, "DEFAULT_TIMEOUT", inside_pos, code).is_some(),
        "DEFAULT_TIMEOUT hover should resolve inside enclosing Config class"
    );

    let outside_pos = Position::new(14, 6);
    assert!(
        find_hover(&index, &url, "DEFAULT_TIMEOUT", outside_pos, code).is_none(),
        "DEFAULT_TIMEOUT hover must NOT resolve outside Config class without receiver"
    );
}

#[test]
fn test_private_top_level_declarations_do_not_leak_to_other_files_in_same_package() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/FileA.kt").unwrap();
    let file_a_code = r#"
package com.example.service

private val SECRET_API_KEY: String = "secret-12345"
private fun internalComputeHash(data: String): String = data
"#;

    let file_b_url = Url::parse("file:///project/FileB.kt").unwrap();
    let file_b_code = r#"
package com.example.service

fun handleRequest() {
    val key = SECRET_API_KEY
    val hash = internalComputeHash("test")
}
"#;

    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos_key = Position::new(4, 15);
    assert!(
        find_definition(&index, &file_b_url, "SECRET_API_KEY", pos_key, file_b_code).is_none(),
        "Private top-level property SECRET_API_KEY must not be resolveable from another file in same package"
    );
    assert!(
        find_hover(&index, &file_b_url, "SECRET_API_KEY", pos_key, file_b_code).is_none(),
        "Private top-level property SECRET_API_KEY must not have hover from another file in same package"
    );

    let pos_hash = Position::new(5, 16);
    assert!(
        find_definition(
            &index,
            &file_b_url,
            "internalComputeHash",
            pos_hash,
            file_b_code
        )
        .is_none(),
        "Private top-level function internalComputeHash must not be resolveable from another file in same package"
    );
    assert!(
        find_hover(
            &index,
            &file_b_url,
            "internalComputeHash",
            pos_hash,
            file_b_code
        )
        .is_none(),
        "Private top-level function internalComputeHash must not have hover from another file in same package"
    );
}
