use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_member_method_named_argument_definition_and_hover() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

class NetworkService {
    /**
     * Send HTTP request.
     * @param url Target server URL
     * @param timeout Request timeout in seconds
     */
    fun sendRequest(url: String, timeout: Int): String = "ok"
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.service.NetworkService

fun test(net: NetworkService) {
    net.sendRequest(url = "https://example.com", timeout = 30)
}
"#;
    index.index_source(service_url.clone(), service_code);
    index.index_source(main_url.clone(), main_code);

    let pos_url = Position::new(6, 20);
    let def = find_definition(&index, &main_url, "url", pos_url, main_code);
    assert!(
        def.is_some(),
        "Named argument 'url' on member call 'net.sendRequest' should resolve definition"
    );
    assert_eq!(def.unwrap().uri, service_url);

    let hover = find_hover(&index, &main_url, "url", pos_url, main_code);
    assert!(
        hover.is_some(),
        "Named argument 'url' on member call 'net.sendRequest' should resolve hover"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(text.contains("url: String"));
            assert!(text.contains("Target server URL"));
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_member_method_call_argument_completion() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

class NetworkService {
    fun sendRequest(url: String, timeout: Int): String = "ok"
}
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.service.NetworkService

fun test(net: NetworkService) {
    net.sendRequest(
}
"#;
    index.index_source(service_url, service_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 20);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"url".to_string()),
        "Completion inside 'net.sendRequest(' should offer parameter 'url'"
    );
    assert!(
        labels.contains(&"timeout".to_string()),
        "Completion inside 'net.sendRequest(' should offer parameter 'timeout'"
    );
}

#[test]
fn test_data_class_copy_named_argument_definition_and_hover() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

/**
 * User representation.
 * @param id Unique identifier
 * @param username Login username
 */
data class User(val id: String, val username: String)
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.User

fun test(u: User) {
    val u2 = u.copy(username = "bob")
}
"#;
    index.index_source(model_url.clone(), model_code);
    index.index_source(main_url.clone(), main_code);

    let pos_username = Position::new(6, 20);
    let def = find_definition(&index, &main_url, "username", pos_username, main_code);
    assert!(
        def.is_some(),
        "Named argument 'username' in 'u.copy(' should resolve definition"
    );
    assert_eq!(def.unwrap().uri, model_url);

    let hover = find_hover(&index, &main_url, "username", pos_username, main_code);
    assert!(
        hover.is_some(),
        "Named argument 'username' in 'u.copy(' should resolve hover"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(text.contains("username: String"));
            assert!(text.contains("Login username"));
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_data_class_copy_argument_completion() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

data class User(val id: String, val username: String)
"#;
    let main_url = Url::parse("file:///project/Main.kt").unwrap();
    let main_code = r#"
package com.example.main

import com.example.model.User

fun test(u: User) {
    val u2 = u.copy(
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(main_url.clone(), main_code);

    let pos = Position::new(6, 20);
    let completions = find_completions(&index, &main_url, pos, main_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"id".to_string()),
        "Completion inside 'u.copy(' should offer property 'id'"
    );
    assert!(
        labels.contains(&"username".to_string()),
        "Completion inside 'u.copy(' should offer property 'username'"
    );
}

#[test]
fn test_call_argument_completion_safe_call_and_instantiation() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Operation.kt").unwrap();
    let source = r#"class Operation(val type: String = "add") {
    fun execute(x: Int, y: Int): Int = TODO()
}

fun main() {
    val opt = Operation()
    opt?.execute()
    Operation().execute()
    Operation()?.execute()
}
"#;
    index.index_source(file_url.clone(), source);

    // 1. opt?.execute() - line 6, col 17
    let pos1 = Position::new(6, 17);
    let completions1 = find_completions(&index, &file_url, pos1, source);
    let labels1: Vec<String> = completions1.into_iter().map(|c| c.label).collect();
    assert!(
        labels1.contains(&"x".to_string()),
        "Completion in opt?.execute( should offer 'x', got: {:?}",
        labels1
    );
    assert!(
        labels1.contains(&"y".to_string()),
        "Completion in opt?.execute( should offer 'y', got: {:?}",
        labels1
    );

    // 2. Operation().execute() - line 7, col 24
    let pos2 = Position::new(7, 24);
    let completions2 = find_completions(&index, &file_url, pos2, source);
    let labels2: Vec<String> = completions2.into_iter().map(|c| c.label).collect();
    assert!(
        labels2.contains(&"x".to_string()),
        "Completion in Operation().execute( should offer 'x', got: {:?}",
        labels2
    );
    assert!(
        labels2.contains(&"y".to_string()),
        "Completion in Operation().execute( should offer 'y', got: {:?}",
        labels2
    );

    // 3. Operation()?.execute() - line 8, col 25
    let pos3 = Position::new(8, 25);
    let completions3 = find_completions(&index, &file_url, pos3, source);
    let labels3: Vec<String> = completions3.into_iter().map(|c| c.label).collect();
    assert!(
        labels3.contains(&"x".to_string()),
        "Completion in Operation()?.execute( should offer 'x', got: {:?}",
        labels3
    );
    assert!(
        labels3.contains(&"y".to_string()),
        "Completion in Operation()?.execute( should offer 'y', got: {:?}",
        labels3
    );
}
