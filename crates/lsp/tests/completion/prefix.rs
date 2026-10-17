use lsp::features::find_completions;
use lsp::indexer::{DeclarationItem, DeclarationMetadata, FileSymbols, WorkspaceIndex};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Documentation, Position, Range, SymbolKind, Url};

fn make_file(pkg: &str, decls: Vec<DeclarationItem>) -> FileSymbols {
    FileSymbols {
        package_name: Some(pkg.to_string()),
        declarations: decls,
        ..Default::default()
    }
}

fn make_fn_decl(name: &str, sig: Option<&str>, doc: Option<&str>) -> DeclarationItem {
    DeclarationItem::new(
        name,
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::new(doc.map(SmolStr::new), sig.map(SmolStr::new), None, false),
    )
}

#[test]
fn test_completion_finds_prefix_match_past_max_earlier_items() {
    let mut index = WorkspaceIndex::new();

    let mut dummy_decls = Vec::new();
    for i in 0..1100 {
        dummy_decls.push(make_fn_decl(&format!("supreme{}", i), None, None));
    }
    let dummy_url = Url::parse("jar:file:///caches/dummy.jar!/com/example/Dummy.class").unwrap();
    index.insert_file_symbols(dummy_url, make_file("com.example", dummy_decls));

    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();
    index.insert_file_symbols(
        class_url,
        make_file(
            "androidx.compose.runtime",
            vec![make_fn_decl("remember", Some("fun remember(): Any"), None)],
        ),
    );

    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();
    index.insert_file_symbols(
        src_url,
        make_file(
            "androidx.compose.runtime",
            vec![make_fn_decl(
                "remember",
                Some("fun <T> remember(calculation: () -> T): T"),
                Some("Remember doc"),
            )],
        ),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    rem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 7), app_code);
    let item = completions
        .iter()
        .find(|c| c.label == "remember")
        .expect("remember should be suggested even if >1000 substring matches precede it");

    assert_eq!(
        item.detail.as_deref(),
        Some("fun <T> remember(calculation: () -> T): T")
    );
    let first = &completions[0];
    assert!(
        first.label.starts_with("rem") || first.label.starts_with("Rem"),
        "Prefix match should be prioritized over pure substring match"
    );
}

#[test]
fn test_completion_prefix_sorted_alphabetically_before_substring_matches() {
    let mut index = WorkspaceIndex::new();

    let decls = vec![
        make_fn_decl("unrelated_foo", None, None),
        make_fn_decl("barFoo", None, None),
        make_fn_decl("fooBeta", None, None),
        make_fn_decl("fooAlpha", None, None),
    ];
    let url = Url::parse("jar:file:///caches/lib.jar!/com/example/Lib.class").unwrap();
    index.insert_file_symbols(url, make_file("com.example", decls));

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    foo\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 7), app_code);
    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

    assert_eq!(
        labels,
        vec!["fooAlpha", "fooBeta", "barFoo", "unrelated_foo"]
    );
}

#[test]
fn test_completion_prefix_saturated_replaces_bytecode_with_source() {
    let mut index = WorkspaceIndex::new();

    let mut bytecode_decls = Vec::new();
    bytecode_decls.push(make_fn_decl(
        "remember",
        Some("fun remember(calculation: () -> Any): Any"),
        None,
    ));
    for i in 0..1050 {
        bytecode_decls.push(make_fn_decl(&format!("remItem{}", i), None, None));
    }

    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();
    index.insert_file_symbols(
        class_url,
        make_file("androidx.compose.runtime", bytecode_decls),
    );

    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();
    index.insert_file_symbols(
        src_url,
        make_file(
            "androidx.compose.runtime",
            vec![make_fn_decl(
                "remember",
                Some("fun <T> remember(calculation: () -> T): T"),
                Some("Remember doc from source"),
            )],
        ),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    rem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 7), app_code);
    let item = completions
        .iter()
        .find(|c| c.label == "remember")
        .expect("remember should be retained and upgraded even when tracker is saturated");

    assert_eq!(
        item.detail.as_deref(),
        Some("fun <T> remember(calculation: () -> T): T")
    );
    if let Some(Documentation::MarkupContent(mc)) = &item.documentation {
        assert_eq!(mc.value, "Remember doc from source");
    } else {
        panic!("expected documentation from source");
    }
}

#[test]
fn test_completion_empty_prefix_returns_up_to_limit() {
    let mut index = WorkspaceIndex::new();
    let mut bytecode_decls = Vec::new();
    for i in 0..10 {
        bytecode_decls.push(make_fn_decl(&format!("item{}", i), None, None));
    }

    let class_url = Url::parse("jar:file:///caches/runtime.aar!/com/example/Lib.class").unwrap();
    index.insert_file_symbols(class_url, make_file("com.example", bytecode_decls));

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    \n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 4), app_code);
    assert!(
        completions.len() >= 10,
        "Expected at least 10 completions on empty prefix, got {}",
        completions.len()
    );
}
