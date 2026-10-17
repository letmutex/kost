use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_cross_file_class_with_primary_constructor_completion() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

/**
 * User profile entity.
 */
class UserProfile(val id: String, val name: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun run() {
    UserP
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(5, 9);
    let completions = find_completions(&index, &file_a_url, pos, file_a_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"UserProfile".to_string()),
        "Completion in file A should offer 'UserProfile' from file B"
    );
}

#[test]
fn test_cross_file_class_with_primary_constructor_call_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

/**
 * User profile entity.
 */
class UserProfile(val id: String, val name: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun run() {
    val u = UserProfile("1", "Alice")
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(5, 16);
    let hover = find_hover(&index, &file_a_url, "UserProfile", pos, file_a_code);
    assert!(
        hover.is_some(),
        "Hover on 'UserProfile' constructor call in file A should resolve"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(
                text.contains("UserProfile"),
                "Hover should contain class name 'UserProfile'"
            );
            assert!(
                text.contains("User profile entity."),
                "Hover should contain class doc comment"
            );
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_cross_file_class_type_reference_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

/**
 * User profile entity.
 */
class UserProfile(val id: String, val name: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun handle(profile: UserProfile) {
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(5, 24);
    let hover = find_hover(&index, &file_a_url, "UserProfile", pos, file_a_code);
    assert!(
        hover.is_some(),
        "Hover on 'UserProfile' type reference in file A should resolve"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(
                text.contains("User profile entity."),
                "Hover should contain class doc comment"
            );
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_cross_file_primary_constructor_member_completion() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

class UserProfile(val id: String, val name: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun test(u: UserProfile) {
    u.
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(6, 6);
    let completions = find_completions(&index, &file_a_url, pos, file_a_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"id".to_string()),
        "Completion on 'u.' in file A should offer primary constructor property 'id'"
    );
    assert!(
        labels.contains(&"name".to_string()),
        "Completion on 'u.' in file A should offer primary constructor property 'name'"
    );
}

#[test]
fn test_cross_file_primary_constructor_member_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

class UserProfile(
    /** The unique identifier of the user */
    val id: String,
    val name: String
)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun test(u: UserProfile) {
    val x = u.id
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(6, 14);
    let hover = find_hover(&index, &file_a_url, "id", pos, file_a_code);
    assert!(
        hover.is_some(),
        "Hover on 'u.id' in file A should resolve member declared in primary constructor in file B"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(
                text.contains("val id: String"),
                "Hover should contain property signature 'val id: String'"
            );
            assert!(
                text.contains("The unique identifier of the user"),
                "Hover should contain property doc comment"
            );
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_cross_file_primary_constructor_named_argument_definition_and_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

/**
 * User profile.
 * @param id Unique identifier
 * @param name Full display name
 */
class UserProfile(val id: String, val name: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun test() {
    val u = UserProfile(id = "123", name = "Alice")
}
"#;
    index.index_source(file_b_url.clone(), file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos_id = Position::new(6, 25);
    let def = find_definition(&index, &file_a_url, "id", pos_id, file_a_code);
    assert!(
        def.is_some(),
        "Named argument 'id' at call site in file A targeting primary constructor in file B should resolve definition"
    );
    assert_eq!(def.unwrap().uri, file_b_url);

    let hover = find_hover(&index, &file_a_url, "id", pos_id, file_a_code);
    assert!(
        hover.is_some(),
        "Named argument 'id' at call site in file A targeting primary constructor in file B should resolve hover"
    );
}

#[test]
fn test_cross_file_primary_constructor_argument_completion() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

class UserProfile(val id: String, val name: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.UserProfile

fun test() {
    val u = UserProfile(
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(6, 24);
    let completions = find_completions(&index, &file_a_url, pos, file_a_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"id".to_string()),
        "Completion inside 'UserProfile(' should offer primary constructor parameter 'id'"
    );
    assert!(
        labels.contains(&"name".to_string()),
        "Completion inside 'UserProfile(' should offer primary constructor parameter 'name'"
    );
}

#[test]
fn test_cross_file_primary_constructor_non_val_parameters_definition_and_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Conn.kt").unwrap();
    let file_b_code = r#"
package com.example.conn

/**
 * Socket connection.
 * @param host Target server host
 * @param port Target server port
 */
class Connection(host: String, port: Int)
"#;
    let file_a_url = Url::parse("file:///project/App.kt").unwrap();
    let file_a_code = r#"
package com.example.app

import com.example.conn.Connection

fun test() {
    val c = Connection(host = "localhost", port = 8080)
}
"#;
    index.index_source(file_b_url.clone(), file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos_host = Position::new(6, 25);
    let def = find_definition(&index, &file_a_url, "host", pos_host, file_a_code);
    assert!(
        def.is_some(),
        "Named argument 'host' targeting non-val primary constructor in file B should resolve definition"
    );
    assert_eq!(def.unwrap().uri, file_b_url);

    let hover = find_hover(&index, &file_a_url, "host", pos_host, file_a_code);
    assert!(
        hover.is_some(),
        "Named argument 'host' targeting non-val primary constructor in file B should resolve hover"
    );
}
