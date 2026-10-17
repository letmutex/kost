use lsp::features::find_completions;
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
fn test_completion_replaces_all_overloaded_bytecode_functions_with_source() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Surface.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Surface.kt").unwrap();

    let class_decl1 = make_fn_decl("surface", Some("fun surface(modifier: Any): Unit"), None);
    let class_decl2 = make_fn_decl(
        "surface",
        Some("fun surface(modifier: Any, opaque: Boolean): Unit"),
        None,
    );

    let src_decl1 = make_fn_decl(
        "surface",
        Some("fun surface(customModifier: String = \"def\"): Unit"),
        Some("Surface 1 source doc"),
    );
    let src_decl2 = make_fn_decl(
        "surface",
        Some("fun surface(customModifier: String = \"def\", isOpaque: Boolean = true): Unit"),
        Some("Surface 2 source doc"),
    );

    index.insert_file_symbols(
        class_url,
        make_file("com.example", vec![class_decl1, class_decl2]),
    );
    index.insert_file_symbols(
        src_url,
        make_file("com.example", vec![src_decl1, src_decl2]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    surf\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 8), app_code);
    let surface_items: Vec<_> = completions
        .into_iter()
        .filter(|c| c.label == "surface")
        .collect();

    assert_eq!(
        surface_items.len(),
        2,
        "Should return both surface overloads"
    );
    assert!(
        surface_items
            .iter()
            .any(|c| c.detail.as_deref()
                == Some("fun surface(customModifier: String = \"def\"): Unit")),
        "First overload should be replaced with source detail"
    );
    assert!(
        surface_items.iter().any(|c| c.detail.as_deref()
            == Some(
                "fun surface(customModifier: String = \"def\", isOpaque: Boolean = true): Unit"
            )),
        "Second overload should be replaced with source detail"
    );
}

#[test]
fn test_completion_bytecode_fewer_source_overloads_retains_unmatched_bytecode() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Format.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Format.kt").unwrap();

    let class_decl1 = make_fn_decl("format", Some("fun format(value: Int): String"), None);
    let class_decl2 = make_fn_decl("format", Some("fun format(value: Double): String"), None);

    let src_decl1 = make_fn_decl(
        "format",
        Some("fun format(value: Int): String"),
        Some("Source doc for Int"),
    );

    index.insert_file_symbols(src_url, make_file("com.example", vec![src_decl1]));
    index.insert_file_symbols(
        class_url,
        make_file("com.example", vec![class_decl1, class_decl2]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    form\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 8), app_code);
    let format_items: Vec<_> = completions
        .into_iter()
        .filter(|c| c.label == "format")
        .collect();

    assert_eq!(
        format_items.len(),
        2,
        "Should return both format overloads (Int from source, Double from bytecode)"
    );
}

#[test]
fn test_completion_bytecode_different_arity_not_replaced_by_single_source_overload() {
    let mut index = WorkspaceIndex::new();
    let class_url = Url::parse("jar:file:///caches/lib.jar!/com/example/Format.class").unwrap();
    let src_url = Url::parse("jar:file:///caches/lib-sources.jar!/com/example/Format.kt").unwrap();

    // Bytecode has 1 overload with 1 param (Double)
    let class_decl = make_fn_decl("format", Some("fun format(value: Double): String"), None);

    // Source has 1 overload with 2 params (String, Int)
    let src_decl = make_fn_decl(
        "format",
        Some("fun format(prefix: String, value: Int): String"),
        Some("Source doc for 2-param format"),
    );

    index.insert_file_symbols(class_url, make_file("com.example", vec![class_decl]));
    index.insert_file_symbols(src_url, make_file("com.example", vec![src_decl]));

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    form\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 8), app_code);
    let format_items: Vec<_> = completions
        .into_iter()
        .filter(|c| c.label == "format")
        .collect();

    assert_eq!(
        format_items.len(),
        2,
        "Should return both overloads because arity differs, but single-element fallback in completion overwrote bytecode"
    );
}

#[test]
fn test_completion_source_indexed_first_suppresses_bytecode_callable_with_compatible_arity() {
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
        Some("Source doc for remember"),
    );
    let class_decl = make_fn_decl(
        "remember",
        Some("fun remember(calculation: () -> Any): Any"),
        None,
    );

    index.insert_file_symbols(
        src_url,
        make_file("androidx.compose.runtime", vec![src_decl]),
    );
    index.insert_file_symbols(
        class_url,
        make_file("androidx.compose.runtime", vec![class_decl]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    rem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 7), app_code);
    let remember_items: Vec<_> = completions
        .into_iter()
        .filter(|c| c.label == "remember")
        .collect();

    assert_eq!(
        remember_items.len(),
        1,
        "Bytecode callable should be suppressed when source callable with matching arity already indexed"
    );
    assert_eq!(
        remember_items[0].detail.as_deref(),
        Some("fun <T> remember(calculation: () -> T): T")
    );
}

#[test]
fn test_completion_subsequent_duplicate_bytecode_suppressed_after_source_replacement() {
    let mut index = WorkspaceIndex::new();
    let class_url1 =
        Url::parse("jar:file:///caches/runtime-1.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();
    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();
    let class_url2 =
        Url::parse("jar:file:///caches/runtime-2.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();

    let class_decl1 = make_fn_decl(
        "remember",
        Some("fun remember(calculation: () -> Any): Any"),
        None,
    );
    let src_decl = make_fn_decl(
        "remember",
        Some("fun <T> remember(calculation: () -> T): T"),
        Some("Source doc for remember"),
    );
    let class_decl2 = make_fn_decl(
        "remember",
        Some("fun remember(calculation: () -> Any): Any"),
        None,
    );

    index.insert_file_symbols(
        class_url1,
        make_file("androidx.compose.runtime", vec![class_decl1]),
    );
    index.insert_file_symbols(
        src_url,
        make_file("androidx.compose.runtime", vec![src_decl]),
    );
    index.insert_file_symbols(
        class_url2,
        make_file("androidx.compose.runtime", vec![class_decl2]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    rem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 7), app_code);
    let remember_items: Vec<_> = completions
        .into_iter()
        .filter(|c| c.label == "remember")
        .collect();

    assert_eq!(
        remember_items.len(),
        1,
        "Subsequent duplicate bytecode must be suppressed after source replaced earlier bytecode"
    );
}

#[test]
fn test_completion_multi_overload_unique_arity_replaces_bytecode() {
    let mut index = WorkspaceIndex::new();
    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/Remember.class")
            .unwrap();
    let src_url = Url::parse(
        "jar:file:///caches/runtime-desktop-sources.jar!/androidx/compose/runtime/Remember.kt",
    )
    .unwrap();

    let class_decl1 = make_fn_decl(
        "remember",
        Some("fun remember(calculation: () -> Any): Any"),
        None,
    );
    let class_decl2 = make_fn_decl(
        "remember",
        Some("fun remember(key: Any, calculation: () -> Any): Any"),
        None,
    );

    let src_decl1 = make_fn_decl(
        "remember",
        Some("fun <T> remember(calculation: () -> T): T"),
        Some("Doc 1-param"),
    );
    let src_decl2 = make_fn_decl(
        "remember",
        Some("fun <T> remember(key1: Any?, calculation: () -> T): T"),
        Some("Doc 2-param"),
    );

    index.insert_file_symbols(
        class_url,
        make_file("androidx.compose.runtime", vec![class_decl1, class_decl2]),
    );
    index.insert_file_symbols(
        src_url,
        make_file("androidx.compose.runtime", vec![src_decl1, src_decl2]),
    );

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nfun test() {\n    rem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(2, 7), app_code);
    let remember_items: Vec<_> = completions
        .into_iter()
        .filter(|c| c.label == "remember")
        .collect();

    assert_eq!(remember_items.len(), 2, "Should return both overloads");
    assert!(
        remember_items
            .iter()
            .any(|c| c.detail.as_deref() == Some("fun <T> remember(calculation: () -> T): T")),
        "1-param overload should be replaced with source detail"
    );
    assert!(
        remember_items.iter().any(|c| c.detail.as_deref()
            == Some("fun <T> remember(key1: Any?, calculation: () -> T): T")),
        "2-param overload should be replaced with source detail"
    );
}
