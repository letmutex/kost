use lsp::config::ServerConfig;
use lsp::features::auto_import::{
    AutoImportData, compute_auto_import_edit, extract_auto_import_data, is_kotlin_default_import,
    resolve_completion_item,
};
use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use serde_json::json;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_auto_import_candidate_carries_import_data() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/OrderService.kt").unwrap();
    let service_code = r#"
package com.example.service

class OrderProcessor {
    fun process(): Boolean = true
}
"#;

    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"
package com.example.client

fun run() {
    Order
}
"#;

    index.index_source(service_url, service_code);
    index.index_source(client_url.clone(), client_code);

    let pos = Position::new(4, 9); // cursor after `Order`
    let items = find_completions(&index, &client_url, pos, client_code);
    let order_item = items
        .iter()
        .find(|item| item.label == "OrderProcessor")
        .expect("OrderProcessor should be offered in completions");

    let data = extract_auto_import_data(order_item)
        .expect("unimported candidate from another package should have auto import data");

    assert_eq!(data.package_name, "com.example.service");
    assert_eq!(data.symbol_name, "OrderProcessor");
    assert_eq!(data.target_uri, client_url);
    assert_eq!(data.full_name(), "com.example.service.OrderProcessor");
}

#[test]
fn test_auto_import_resolve_adds_additional_text_edits() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

fun run() {
    Order
}
"#;

    let data = AutoImportData::new(
        client_url,
        "com.example.service",
        "OrderProcessor",
        None::<String>,
    );

    let dummy_item = tower_lsp::lsp_types::CompletionItem {
        label: "OrderProcessor".to_string(),
        data: serde_json::to_value(&data).ok(),
        ..Default::default()
    };

    let resolved = resolve_completion_item(dummy_item, client_code, &data);
    let edits = resolved
        .additional_text_edits
        .expect("resolved item should contain additionalTextEdits");

    assert_eq!(edits.len(), 1);
    assert!(
        edits[0]
            .new_text
            .contains("import com.example.service.OrderProcessor"),
        "edit must insert import for target class: {:?}",
        edits[0].new_text
    );
}

#[test]
fn test_auto_import_suppressed_for_same_package() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/A.kt").unwrap();
    let file_a_code = r#"
package com.example.common

class SharedHelper
"#;

    let file_b_url = Url::parse("file:///project/B.kt").unwrap();
    let file_b_code = r#"
package com.example.common

fun test() {
    Shared
}
"#;

    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url.clone(), file_b_code);

    let pos = Position::new(4, 10);
    let items = find_completions(&index, &file_b_url, pos, file_b_code);
    let shared_item = items
        .iter()
        .find(|item| item.label == "SharedHelper")
        .expect("SharedHelper should be offered in completions");

    assert!(
        shared_item.data.is_none(),
        "candidate from same package must not carry auto import data: {:?}",
        shared_item.data
    );

    let data = AutoImportData::new(
        file_b_url,
        "com.example.common",
        "SharedHelper",
        None::<String>,
    );
    let edit = compute_auto_import_edit(file_b_code, &data);
    assert!(
        edit.is_none(),
        "same package import should be suppressed by compute_auto_import_edit"
    );
}

#[test]
fn test_auto_import_suppressed_for_kotlin_stdlib_defaults() {
    assert!(is_kotlin_default_import("kotlin"));
    assert!(is_kotlin_default_import("kotlin.collections"));
    assert!(is_kotlin_default_import("kotlin.ranges"));
    assert!(is_kotlin_default_import("kotlin.text"));
    assert!(is_kotlin_default_import("java.lang"));
    assert!(!is_kotlin_default_import("kotlinx.coroutines"));
    assert!(!is_kotlin_default_import("com.example.util"));

    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

fun run() {}
"#;
    let data = AutoImportData::new(client_url, "kotlin.collections", "List", None::<String>);

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_none(),
        "default kotlin package must not generate auto import edit"
    );
}

#[test]
fn test_auto_import_suppressed_when_already_imported() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

import com.example.service.OrderProcessor

fun run() {}
"#;
    let data = AutoImportData::new(
        client_url,
        "com.example.service",
        "OrderProcessor",
        None::<String>,
    );

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_none(),
        "already imported class must not generate auto import edit"
    );
}

#[test]
fn test_auto_import_suppressed_when_wildcard_imported() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

import com.example.service.*

fun run() {}
"#;
    let data = AutoImportData::new(
        client_url,
        "com.example.service",
        "OrderProcessor",
        None::<String>,
    );

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_none(),
        "wildcard imported package must not generate auto import edit"
    );
}

#[test]
fn test_auto_import_not_suppressed_for_aliased_import() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

import com.example.service.OrderProcessor as MyProcessor

fun run() {}
"#;
    let data = AutoImportData::new(
        client_url,
        "com.example.service",
        "OrderProcessor",
        None::<String>,
    );

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_some(),
        "aliased import must not suppress unaliased auto import"
    );
}

#[test]
fn test_auto_import_suppressed_for_multiline_package_header() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = "package\n    com.example.service\n\nfun run() {}\n";
    let data = AutoImportData::new(
        client_url,
        "com.example.service",
        "OrderProcessor",
        None::<String>,
    );

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_none(),
        "multiline package header must suppress same package import"
    );
}

#[test]
fn test_auto_import_disambiguation_for_same_named_classes_in_different_packages() {
    let mut index = WorkspaceIndex::new();
    let pkg_a_url = Url::parse("file:///project/UiComponent.kt").unwrap();
    let pkg_a_code = "package com.example.ui\n\nclass Component\n";

    let pkg_b_url = Url::parse("file:///project/ReflectComponent.kt").unwrap();
    let pkg_b_code = "package com.example.reflect\n\nclass Component\n";

    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = "package com.example.client\n\nfun run() {\n    Compon\n}\n";

    index.index_source(pkg_a_url, pkg_a_code);
    index.index_source(pkg_b_url, pkg_b_code);
    index.index_source(client_url.clone(), client_code);

    let pos = Position::new(3, 10);
    let items = find_completions(&index, &client_url, pos, client_code);
    let components: Vec<_> = items
        .into_iter()
        .filter(|item| item.label == "Component")
        .collect();
    assert_eq!(
        components.len(),
        2,
        "should offer Component from both com.example.ui and com.example.reflect"
    );
}

#[test]
fn test_auto_import_disabled_via_config_option() {
    let config_default = ServerConfig::default();
    assert!(config_default.auto_import);

    let opts_disabled = json!({
        "auto_import": false
    });
    let config_disabled = ServerConfig::from_init_options(Some(&opts_disabled));
    assert!(!config_disabled.auto_import);

    let opts_alias = json!({
        "completion_auto_import": "false"
    });
    let config_alias = ServerConfig::from_init_options(Some(&opts_alias));
    assert!(!config_alias.auto_import);
}
