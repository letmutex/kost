use lsp::features::auto_import::{
    AutoImportData, compute_auto_import_edit, resolve_completion_item,
};
use lsp::features::completion::find_completions_with_config;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_auto_import_suppressed_for_container_wildcard_import() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

import com.example.container.Outer.*

fun run() {}
"#;
    let data = AutoImportData::new(client_url, "com.example.container", "Inner", Some("Outer"));

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_none(),
        "container wildcard import should suppress auto-import for nested member"
    );
}

#[test]
fn test_auto_import_not_suppressed_for_package_wildcard_when_symbol_is_nested() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = r#"package com.example.client

import com.example.container.*

fun run() {}
"#;
    let data = AutoImportData::new(client_url, "com.example.container", "Inner", Some("Outer"));

    let edit = compute_auto_import_edit(client_code, &data);
    assert!(
        edit.is_some(),
        "package wildcard does not import nested members, so auto-import should not be suppressed"
    );
}

#[test]
fn test_auto_import_disabled_via_config_suppresses_import_data_in_completion() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/OrderService.kt").unwrap();
    let service_code = r#"
package com.example.service

class OrderProcessor
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

    let pos = Position::new(4, 9);
    let items_disabled =
        find_completions_with_config(&index, &client_url, pos, client_code, None, false);
    let order_item_disabled = items_disabled
        .iter()
        .find(|item| item.label == "OrderProcessor")
        .expect("OrderProcessor should still be offered as candidate");

    assert!(
        order_item_disabled.data.is_none(),
        "when auto_import is disabled, candidates must not have auto import data attached"
    );

    let items_enabled =
        find_completions_with_config(&index, &client_url, pos, client_code, None, true);
    let order_item_enabled = items_enabled
        .iter()
        .find(|item| item.label == "OrderProcessor")
        .expect("OrderProcessor should be offered as candidate");

    assert!(
        order_item_enabled.data.is_some(),
        "when auto_import is enabled, candidates from other packages must have auto import data"
    );
}

#[test]
fn test_auto_import_candidate_label_details_includes_package_and_signature() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/OrderService.kt").unwrap();
    let service_code = r#"
package com.example.service

class OrderProcessor
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

    let pos = Position::new(4, 9);
    let items = find_completions_with_config(&index, &client_url, pos, client_code, None, true);
    let order_item = items
        .iter()
        .find(|item| item.label == "OrderProcessor")
        .expect("OrderProcessor should be offered in completions");

    let label_details = order_item
        .label_details
        .as_ref()
        .expect("auto import candidate must have label_details populated");
    assert_eq!(
        label_details.description.as_deref(),
        Some("com.example.service"),
        "label_details.description must show package name for disambiguation: {:?}",
        label_details
    );
    assert_eq!(
        order_item.detail.as_deref(),
        Some("class OrderProcessor"),
        "detail must retain signature: {:?}",
        order_item.detail
    );
}

#[test]
fn test_auto_import_resolve_idempotent() {
    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = "package com.example.client\n\nfun run() {}\n";
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

    let resolved_first = resolve_completion_item(dummy_item, client_code, &data);
    assert_eq!(
        resolved_first.additional_text_edits.as_ref().map(Vec::len),
        Some(1)
    );

    let resolved_second = resolve_completion_item(resolved_first, client_code, &data);
    assert_eq!(
        resolved_second.additional_text_edits.as_ref().map(Vec::len),
        Some(1),
        "subsequent resolve calls must not duplicate additional text edits"
    );
}

#[test]
fn test_auto_import_multiline_package_header_indexed_properly() {
    let mut index = WorkspaceIndex::new();
    let service_url = Url::parse("file:///project/OrderService.kt").unwrap();
    let service_code = "package\n    com.example.service\n\nclass OrderProcessor\n";

    let client_url = Url::parse("file:///project/Client.kt").unwrap();
    let client_code = "package com.example.client\n\nfun run() {\n    Order\n}\n";

    index.index_source(service_url, service_code);
    index.index_source(client_url.clone(), client_code);

    let pos = Position::new(3, 9);
    let items = find_completions_with_config(&index, &client_url, pos, client_code, None, true);
    let order_item = items
        .iter()
        .find(|item| item.label == "OrderProcessor")
        .expect("OrderProcessor from multiline package header must be suggested");

    let label_details = order_item
        .label_details
        .as_ref()
        .expect("candidate from multiline package header must have label_details");
    assert_eq!(
        label_details.description.as_deref(),
        Some("com.example.service"),
        "package name must be correctly extracted from multiline package header"
    );
}
