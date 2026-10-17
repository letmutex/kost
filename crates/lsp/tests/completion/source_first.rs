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
fn test_scope_completion_prefers_source_over_bytecode() {
    let mut index = WorkspaceIndex::new();
    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();
    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();

    let class_decl = make_fn_decl("remember", Some("fun remember(): Any"), None);
    let src_decl = make_fn_decl(
        "remember",
        Some("fun <T> remember(calculation: () -> T): T"),
        Some("Remember doc"),
    );

    index.insert_file_symbols(
        class_url,
        make_file("androidx.compose.runtime", vec![class_decl]),
    );
    index.insert_file_symbols(
        src_url,
        make_file("androidx.compose.runtime", vec![src_decl]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    remem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 9), app_code);
    let item = completions
        .into_iter()
        .find(|c| c.label == "remember")
        .expect("remember should be suggested");

    assert_eq!(
        item.detail.as_deref(),
        Some("fun <T> remember(calculation: () -> T): T")
    );
    if let Some(Documentation::MarkupContent(mc)) = item.documentation {
        assert_eq!(mc.value, "Remember doc");
    } else {
        panic!("expected documentation from source");
    }
}

#[test]
fn test_member_completion_prefers_source_over_bytecode() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Service.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Service.kt").unwrap();

    let mut class_member = make_fn_decl("doWork", Some("fun doWork(): Unit"), None);
    class_member.container_name = Some(SmolStr::new("Service"));
    let mut src_member = make_fn_decl(
        "doWork",
        Some("fun doWork(timeoutMs: Long): Boolean"),
        Some("Service worker doc"),
    );
    src_member.container_name = Some(SmolStr::new("Service"));

    let mut cls = make_fn_decl("Service", None, None);
    cls.kind = SymbolKind::CLASS;

    index.insert_file_symbols(
        class_url,
        make_file("com.example", vec![cls.clone(), class_member]),
    );
    index.insert_file_symbols(src_url, make_file("com.example", vec![cls, src_member]));

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test(s: Service) {\n    s.do\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 8), app_code);
    let item = completions
        .into_iter()
        .find(|c| c.label == "doWork")
        .expect("doWork should be suggested");

    assert_eq!(
        item.detail.as_deref(),
        Some("fun doWork(timeoutMs: Long): Boolean")
    );
    if let Some(Documentation::MarkupContent(mc)) = item.documentation {
        assert_eq!(mc.value, "Service worker doc");
    } else {
        panic!("expected documentation from source");
    }
}

#[test]
fn test_package_completion_prefers_source_over_bytecode() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/pkg/Util.class").unwrap();
    let src_url =
        Url::parse("jar:file:///caches/lib-sources.jar!/com/example/pkg/Util.kt").unwrap();

    let class_decl = make_fn_decl("helper", Some("fun helper(): Unit"), None);
    let src_decl = make_fn_decl(
        "helper",
        Some("fun helper(v: Int): Int"),
        Some("Helper doc"),
    );

    index.insert_file_symbols(class_url, make_file("com.example.pkg", vec![class_decl]));
    index.insert_file_symbols(src_url, make_file("com.example.pkg", vec![src_decl]));

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package app\nfun test() {\n    com.example.pkg.hel\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 23), app_code);
    let item = completions
        .into_iter()
        .find(|c| c.label == "helper")
        .expect("helper should be suggested");

    assert_eq!(item.detail.as_deref(), Some("fun helper(v: Int): Int"));
    if let Some(Documentation::MarkupContent(mc)) = item.documentation {
        assert_eq!(mc.value, "Helper doc");
    } else {
        panic!("expected documentation from source");
    }
}

#[test]
fn test_completion_retains_source_when_indexed_first() {
    let mut index = WorkspaceIndex::new();
    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();
    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();

    let src_decl = make_fn_decl(
        "remember",
        Some("fun <T> remember(calculation: () -> T): T"),
        Some("Source doc"),
    );
    let class_decl = make_fn_decl("remember", Some("fun remember(): Any"), None);

    // Source inserted FIRST
    index.insert_file_symbols(
        src_url,
        make_file("androidx.compose.runtime", vec![src_decl]),
    );
    // Bytecode inserted SECOND
    index.insert_file_symbols(
        class_url,
        make_file("androidx.compose.runtime", vec![class_decl]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    remem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 9), app_code);
    let item = completions
        .into_iter()
        .find(|c| c.label == "remember")
        .expect("remember should be suggested");

    assert_eq!(
        item.detail.as_deref(),
        Some("fun <T> remember(calculation: () -> T): T")
    );
    if let Some(Documentation::MarkupContent(mc)) = item.documentation {
        assert_eq!(mc.value, "Source doc");
    } else {
        panic!("expected documentation from source");
    }
}
