use lsp::features::{find_completions, find_definition};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_fqn_type_reference_definition() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class Client(val id: String)
"#;
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

fun handle(client: com.example.model.Client) {
    val id = client.id
}
"#;
    let other_url = Url::parse("file:///project/Other.kt").unwrap();
    let other_code = r#"
package com.example.other

class Client(val otherId: String)
"#;
    index.index_source(other_url, other_code);
    index.index_source(model_url.clone(), model_code);
    index.index_source(service_url.clone(), service_code);

    let pos = Position::new(3, 37);
    let def = find_definition(&index, &service_url, "Client", pos, service_code);
    assert!(
        def.is_some(),
        "Fully-qualified type reference 'com.example.model.Client' should resolve to definition"
    );
    assert_eq!(def.unwrap().uri, model_url);
}

#[test]
fn test_fqn_import_definition() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class Client(val id: String)
"#;
    let service_url = Url::parse("file:///project/Service.kt").unwrap();
    let service_code = r#"
package com.example.service

import com.example.model.Client

fun test(c: Client) {}
"#;
    index.index_source(model_url.clone(), model_code);
    index.index_source(service_url.clone(), service_code);

    let pos = Position::new(3, 26);
    let def = find_definition(&index, &service_url, "Client", pos, service_code);
    assert!(
        def.is_some(),
        "Import statement 'import com.example.model.Client' should resolve to definition"
    );
    assert_eq!(def.unwrap().uri, model_url);
}

#[test]
fn test_fqn_top_level_function_definition() {
    let mut index = WorkspaceIndex::new();
    let util_url = Url::parse("file:///project/Utils.kt").unwrap();
    let util_code = r#"
package com.example.util

fun formatTimestamp(ts: Long): String = ts.toString()
"#;
    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

fun run() {
    val s = com.example.util.formatTimestamp(12345L)
}
"#;
    index.index_source(util_url.clone(), util_code);
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(4, 30);
    let def = find_definition(&index, &app_url, "formatTimestamp", pos, app_code);
    assert!(
        def.is_some(),
        "Fully-qualified function call 'com.example.util.formatTimestamp' should resolve to definition"
    );
    assert_eq!(def.unwrap().uri, util_url);
}

#[test]
fn test_fqn_package_member_completion() {
    let mut index = WorkspaceIndex::new();
    let model_url = Url::parse("file:///project/Model.kt").unwrap();
    let model_code = r#"
package com.example.model

class Client(val id: String)
class Config(val path: String)
"#;
    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = r#"
package com.example.app

fun run() {
    com.example.model.
}
"#;
    index.index_source(model_url, model_code);
    index.index_source(app_url.clone(), app_code);

    let pos = Position::new(4, 22);
    let completions = find_completions(&index, &app_url, pos, app_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();
    assert!(
        labels.contains(&"Client".to_string()),
        "Completion on 'com.example.model.' should offer class 'Client'"
    );
    assert!(
        labels.contains(&"Config".to_string()),
        "Completion on 'com.example.model.' should offer class 'Config'"
    );
}
