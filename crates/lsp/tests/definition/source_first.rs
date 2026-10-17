use lsp::features::find_definition;
use lsp::indexer::{DeclarationItem, DeclarationMetadata, FileSymbols, WorkspaceIndex};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{Position, Range, SymbolKind, Url};

fn make_file(pkg: &str, decls: Vec<DeclarationItem>) -> FileSymbols {
    FileSymbols {
        package_name: Some(pkg.to_string()),
        declarations: decls,
        ..Default::default()
    }
}

fn make_fn(name: &str, container: Option<&str>) -> DeclarationItem {
    DeclarationItem::new(
        name,
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::new(None, None, container.map(SmolStr::new), false),
    )
}

#[test]
fn test_definition_prefers_source_over_bytecode_toplevel() {
    let mut index = WorkspaceIndex::new();
    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();
    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();

    index.insert_file_symbols(
        class_url,
        make_file("androidx.compose.runtime", vec![make_fn("remember", None)]),
    );
    index.insert_file_symbols(
        src_url.clone(),
        make_file("androidx.compose.runtime", vec![make_fn("remember", None)]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nimport androidx.compose.runtime.remember\nfun test() {\n    remember()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let def = find_definition(&index, &app_url, "remember", Position::new(3, 4), app_code);
    assert_eq!(def.map(|l| l.uri), Some(src_url));
}

#[test]
fn test_definition_prefers_source_over_bytecode_member() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Service.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Service.kt").unwrap();

    let mut class_cls = make_fn("Service", None);
    class_cls.kind = SymbolKind::CLASS;
    let mut src_cls = make_fn("Service", None);
    src_cls.kind = SymbolKind::CLASS;

    index.insert_file_symbols(
        class_url,
        make_file(
            "com.example",
            vec![class_cls, make_fn("doWork", Some("com.example.Service"))],
        ),
    );
    index.insert_file_symbols(
        src_url.clone(),
        make_file(
            "com.example",
            vec![src_cls, make_fn("doWork", Some("com.example.Service"))],
        ),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test(s: Service) {\n    s.doWork()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let def = find_definition(&index, &app_url, "doWork", Position::new(2, 6), app_code);
    assert_eq!(def.map(|l| l.uri), Some(src_url));
}

#[test]
fn test_definition_prefers_source_over_bytecode_explicit_import() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Action.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Action.kt").unwrap();

    index.insert_file_symbols(
        class_url,
        make_file("com.example", vec![make_fn("action", None)]),
    );
    index.insert_file_symbols(
        src_url.clone(),
        make_file("com.example", vec![make_fn("action", None)]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example.app\nimport com.example.action\nfun test() {}\n";
    index.index_source(app_url.clone(), app_code);

    let def = find_definition(&index, &app_url, "action", Position::new(1, 20), app_code);
    assert_eq!(def.map(|l| l.uri), Some(src_url));
}

#[test]
fn test_definition_prefers_source_over_bytecode_package_call() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Action.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Action.kt").unwrap();

    index.insert_file_symbols(
        class_url,
        make_file("com.example", vec![make_fn("action", None)]),
    );
    index.insert_file_symbols(
        src_url.clone(),
        make_file("com.example", vec![make_fn("action", None)]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package app\nfun test() {\n    com.example.action()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let def = find_definition(&index, &app_url, "action", Position::new(2, 17), app_code);
    assert_eq!(def.map(|l| l.uri), Some(src_url));
}

#[test]
fn test_definition_falls_back_to_bytecode_when_no_source() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Action.class").unwrap();

    index.insert_file_symbols(
        class_url.clone(),
        make_file("com.example", vec![make_fn("action", None)]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    action()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let def = find_definition(&index, &app_url, "action", Position::new(2, 4), app_code);
    assert_eq!(def.map(|l| l.uri), Some(class_url));
}
