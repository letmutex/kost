use lsp::features::{find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkedString, Position, Url};

#[test]
fn test_index_and_kdoc_hover() {
    let mut index = WorkspaceIndex::new();
    let url = Url::parse("file:///project/Service.kt").unwrap();
    let code = r#"
package com.example.service

/**
 * Service for handling user profiles.
 */
class UserService {
    /**
     * Finds a user by ID.
     */
    fun findUser(id: String): User {
        return User(id)
    }
}
"#;
    index.index_source(url.clone(), code);

    let file = index.files.get(&url).unwrap();
    assert_eq!(file.package_name.as_deref(), Some("com.example.service"));

    // Check hover on UserService
    let hover = find_hover(&index, &url, "UserService", Position::new(6, 8), code);
    assert!(hover.is_some());
    if let Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(doc)),
        ..
    }) = hover
    {
        assert!(doc.contains("Service for handling user profiles."));
    } else {
        panic!("Expected markdown hover content");
    }

    // Check hover on findUser
    let fn_hover = find_hover(&index, &url, "findUser", Position::new(10, 8), code);
    assert!(fn_hover.is_some());
    if let Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(doc)),
        ..
    }) = fn_hover
    {
        assert!(doc.contains("Finds a user by ID."));
    } else {
        panic!("Expected markdown hover content");
    }
}

#[test]
fn test_member_call_definition_with_same_name() {
    let mut index = WorkspaceIndex::new();
    let dao_url = Url::parse("file:///project/MessageDao.kt").unwrap();
    let dao_code = r#"
package com.example.dao

class MessageDao {
    /**
     * Retrieves all messages from the database.
     */
    fun getAllMessages(): List<String> = emptyList()
}
"#;
    let repo_url = Url::parse("file:///project/MessageRepository.kt").unwrap();
    let repo_code = r#"
package com.example.repo

import com.example.dao.MessageDao

class MessageRepository(val messageDao: MessageDao) {
    fun getAllMessages() {
        messageDao.getAllMessages()
    }
}
"#;
    index.index_source(dao_url.clone(), dao_code);
    index.index_source(repo_url.clone(), repo_code);

    // Hover / Go-to on messageDao.getAllMessages() at line 7, col 23
    let def = find_definition(
        &index,
        &repo_url,
        "getAllMessages",
        Position::new(7, 23),
        repo_code,
    );
    assert!(def.is_some());
    assert_eq!(def.unwrap().uri, dao_url);

    let hover = find_hover(
        &index,
        &repo_url,
        "getAllMessages",
        Position::new(7, 23),
        repo_code,
    );
    assert!(hover.is_some());
    if let Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(doc)),
        ..
    }) = hover
    {
        assert!(doc.contains("Retrieves all messages from the database."));
    } else {
        panic!("Expected markdown hover content");
    }
}
