use lsp::features::find_hover;
use lsp::indexer::{DeclarationItem, DeclarationMetadata, FileSymbols, WorkspaceIndex};
use smol_str::SmolStr;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Range, SymbolKind, Url};

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
        Range::new(Position::new(1, 0), Position::new(3, 1)),
        Range::new(Position::new(1, 4), Position::new(1, 4 + name.len() as u32)),
        DeclarationMetadata::new(doc.map(SmolStr::new), sig.map(SmolStr::new), None, false),
    )
}

#[test]
fn test_hover_prefers_source_over_bytecode_doc_and_signature() {
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
        Some("Remember a value across recompositions."),
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
    let app_code = "package com.example\nimport androidx.compose.runtime.remember\nfun test() {\n    remember()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let hover = find_hover(&index, &app_url, "remember", Position::new(3, 4), app_code)
        .expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("Remember a value across recompositions."),
            "Doc from .kt should be present: {}",
            text
        );
        assert!(
            text.contains("fun <T> remember(calculation: () -> T): T"),
            "Signature from .kt should be present: {}",
            text
        );
        assert!(
            text.contains("runtime-desktop-sources.jar"),
            "Footer should link to sources jar: {}",
            text
        );
    } else {
        panic!("unexpected hover contents");
    }
}

#[test]
fn test_hover_member_prefers_source_over_bytecode() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Service.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Service.kt").unwrap();

    let mut class_member = make_fn_decl("doWork", Some("fun doWork(): Unit"), None);
    class_member.container_name = Some(SmolStr::new("Service"));
    let mut src_member = make_fn_decl(
        "doWork",
        Some("fun doWork(timeoutMs: Long): Boolean"),
        Some("Performs background work with timeout."),
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
    let app_code = "package com.example\nfun test(s: Service) {\n    s.doWork()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let hover = find_hover(&index, &app_url, "doWork", Position::new(2, 6), app_code)
        .expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("Performs background work with timeout."),
            "Doc from .kt should be present: {}",
            text
        );
        assert!(
            text.contains("fun doWork(timeoutMs: Long): Boolean"),
            "Signature from .kt should be present: {}",
            text
        );
        assert!(
            text.contains("lib-sources.jar"),
            "Footer should link to sources jar: {}",
            text
        );
    } else {
        panic!("unexpected hover contents");
    }
}

#[test]
fn test_hover_falls_back_to_bytecode_when_no_source() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Util.class").unwrap();
    let class_decl = make_fn_decl("helper", Some("fun helper(): Unit"), None);

    index.insert_file_symbols(class_url, make_file("com.example", vec![class_decl]));

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    helper()\n}\n";
    index.index_source(app_url.clone(), app_code);

    let hover = find_hover(&index, &app_url, "helper", Position::new(2, 4), app_code)
        .expect("hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(text.contains("fun helper(): Unit"));
        assert!(text.contains("Util.class"));
    } else {
        panic!("unexpected hover contents");
    }
}
