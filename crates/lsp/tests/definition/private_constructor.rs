use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_cross_file_class_with_private_constructor_fields_completion_and_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

/**
 * Service with private credentials.
 * @param apiKey Secret key for API access
 * @param endpoint Remote service URL
 */
class SecureClient(private val apiKey: String, val endpoint: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.SecureClient

fun test() {
    val c = SecureClient("key", "https://api.example.com")
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos_call = Position::new(6, 16);
    let hover = find_hover(&index, &file_a_url, "SecureClient", pos_call, file_a_code);
    assert!(
        hover.is_some(),
        "Hover on 'SecureClient' with private constructor fields in file A should resolve"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(text.contains("SecureClient"));
            assert!(text.contains("Service with private credentials."));
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_cross_file_private_constructor_field_named_argument_definition_and_hover() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

/**
 * Service with private credentials.
 * @param apiKey Secret key for API access
 * @param endpoint Remote service URL
 */
class SecureClient(private val apiKey: String, val endpoint: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.SecureClient

fun test() {
    val c = SecureClient(apiKey = "key", endpoint = "https://api.example.com")
}
"#;
    index.index_source(file_b_url.clone(), file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos_key = Position::new(6, 26);
    let def = find_definition(&index, &file_a_url, "apiKey", pos_key, file_a_code);
    assert!(
        def.is_some(),
        "Named argument 'apiKey' targeting private constructor field in file B should resolve definition"
    );
    assert_eq!(def.unwrap().uri, file_b_url);

    let hover = find_hover(&index, &file_a_url, "apiKey", pos_key, file_a_code);
    assert!(
        hover.is_some(),
        "Named argument 'apiKey' targeting private constructor field in file B should resolve hover"
    );
    if let Some(h) = hover {
        if let HoverContents::Scalar(MarkedString::String(text)) = h.contents {
            assert!(
                text.contains("apiKey: String"),
                "Hover should contain parameter signature"
            );
            assert!(
                text.contains("Secret key for API access"),
                "Hover should contain @param doc description"
            );
        } else {
            panic!("Expected scalar hover markdown");
        }
    }
}

#[test]
fn test_cross_file_private_constructor_field_argument_completion() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

class SecureClient(private val apiKey: String, val endpoint: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.SecureClient

fun test() {
    val c = SecureClient(
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(6, 25);
    let completions = find_completions(&index, &file_a_url, pos, file_a_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"apiKey".to_string()),
        "Completion inside 'SecureClient(' in file A should offer private constructor field 'apiKey'"
    );
    assert!(
        labels.contains(&"endpoint".to_string()),
        "Completion inside 'SecureClient(' in file A should offer constructor field 'endpoint'"
    );
}

#[test]
fn test_cross_file_instance_member_completion_excludes_private_constructor_fields() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Model.kt").unwrap();
    let file_b_code = r#"
package com.example.model

class SecureClient(private val apiKey: String, val endpoint: String)
"#;
    let file_a_url = Url::parse("file:///project/Service.kt").unwrap();
    let file_a_code = r#"
package com.example.service

import com.example.model.SecureClient

fun test(c: SecureClient) {
    c.
}
"#;
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos = Position::new(6, 6);
    let completions = find_completions(&index, &file_a_url, pos, file_a_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"endpoint".to_string()),
        "Completion on 'c.' should offer public property 'endpoint'"
    );
    assert!(
        !labels.contains(&"apiKey".to_string()),
        "Completion on 'c.' in another file must NOT offer private property 'apiKey'"
    );
}

#[test]
fn test_cross_file_class_with_only_private_constructor_fields() {
    let mut index = WorkspaceIndex::new();
    let file_b_url = Url::parse("file:///project/Worker.kt").unwrap();
    let file_b_code = r#"
package com.example.worker

/**
 * Worker with internal secret.
 * @param secret Internal secret
 */
class SecretWorker(private val secret: String)
"#;
    let file_a_url = Url::parse("file:///project/Main.kt").unwrap();
    let file_a_code = r#"
package com.example.main

import com.example.worker.SecretWorker

fun test() {
    val w = SecretWorker(secret = "pass")
}
"#;
    index.index_source(file_b_url.clone(), file_b_code);
    index.index_source(file_a_url.clone(), file_a_code);

    let pos_call = Position::new(6, 16);
    let hover_class = find_hover(&index, &file_a_url, "SecretWorker", pos_call, file_a_code);
    assert!(
        hover_class.is_some(),
        "Hover on 'SecretWorker' with only private constructor fields should resolve"
    );

    let pos_secret = Position::new(6, 26);
    let def = find_definition(&index, &file_a_url, "secret", pos_secret, file_a_code);
    assert!(
        def.is_some(),
        "Named argument 'secret' targeting private constructor field should resolve definition"
    );
    assert_eq!(def.unwrap().uri, file_b_url);

    let hover_param = find_hover(&index, &file_a_url, "secret", pos_secret, file_a_code);
    assert!(
        hover_param.is_some(),
        "Named argument 'secret' targeting private constructor field should resolve hover"
    );
}
