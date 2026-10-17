use lsp::features::{find_completions, find_hover};
use lsp::gradle::class_file::parse_class_file_symbols;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_class_file_top_level_function_has_no_container() {
    let class_bytes = include_bytes!("resources/RememberKt.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse valid class file");

    assert_eq!(
        file_symbols.package_name.as_deref(),
        Some("androidx.compose.runtime")
    );

    let remember_decls: Vec<_> = file_symbols
        .declarations
        .iter()
        .filter(|d| d.name == "remember")
        .collect();
    assert_eq!(
        remember_decls.len(),
        2,
        "Both remember overloads should be parsed"
    );

    for decl in remember_decls {
        assert_eq!(
            decl.container_name, None,
            "Top-level function in *Kt class file should have no container name"
        );
    }
}

#[test]
fn test_class_file_top_level_function_scope_completion() {
    let mut index = WorkspaceIndex::new();
    let class_bytes = include_bytes!("resources/RememberKt.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse valid class file");

    let class_url =
        Url::parse("jar:file:///caches/runtime.aar!/androidx/compose/runtime/RememberKt.class")
            .unwrap();
    index.insert_file_symbols(class_url, file_symbols);

    let app_url = Url::parse("file:///project/App.kt").unwrap();
    let app_code = "package com.example\nimport androidx.compose.runtime.remember\n\nfun test() {\n    remem\n}\n";
    index.index_source(app_url.clone(), app_code);

    let completions = find_completions(&index, &app_url, Position::new(4, 9), app_code);
    let item = completions.into_iter().find(|c| c.label == "remember");
    assert!(
        item.is_some(),
        "remember should be in completion list with import"
    );

    let app_code_no_import = "package com.example\n\nfun test() {\n    remem\n}\n";
    index.index_source(app_url.clone(), app_code_no_import);
    let completions_no_import =
        find_completions(&index, &app_url, Position::new(3, 9), app_code_no_import);
    let item_no_import = completions_no_import
        .into_iter()
        .find(|c| c.label == "remember");
    assert!(
        item_no_import.is_some(),
        "remember should be in auto-import completion list without import"
    );

    let app_code_hover = "package com.example\nimport androidx.compose.runtime.remember\n\nfun test() {\n    remember()\n}\n";
    index.index_source(app_url.clone(), app_code_hover);
    let hover = find_hover(
        &index,
        &app_url,
        "remember",
        Position::new(4, 8),
        app_code_hover,
    );
    assert!(hover.is_some(), "hover on remember should resolve");
}

#[test]
fn test_class_file_regular_kotlin_class_preserves_container() {
    let class_bytes = include_bytes!("resources/MyService.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse regular class file");

    assert_eq!(
        file_symbols.package_name.as_deref(),
        Some("com.example.service")
    );
    let class_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "MyService");
    assert!(
        class_decl.is_some(),
        "Class MyService should be preserved in declarations"
    );

    let method_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "doWork")
        .expect("doWork should be parsed");
    assert_eq!(
        method_decl.container_name.as_deref(),
        Some("MyService"),
        "Member function should retain class container name"
    );
}

#[test]
fn test_class_file_multifile_facade_drops_container() {
    let class_bytes = include_bytes!("resources/Utils.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse multifile facade class file");

    assert_eq!(
        file_symbols.package_name.as_deref(),
        Some("com.example.utils")
    );
    let class_decl = file_symbols.declarations.iter().find(|d| d.name == "Utils");
    assert!(
        class_decl.is_none(),
        "Multifile facade class Utils should not be in declarations"
    );

    let func_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "utilFunc")
        .expect("utilFunc should be parsed");
    assert_eq!(
        func_decl.container_name, None,
        "Top-level function in multifile facade should have no container name"
    );
}

#[test]
fn test_class_file_multifile_part_is_skipped() {
    let class_bytes = include_bytes!("resources/Utils__UtilsKt.class");
    let file_symbols = parse_class_file_symbols(class_bytes);
    assert!(
        file_symbols.is_none(),
        "Multifile class part (k = 5) should be skipped to prevent duplicate symbols"
    );
}

#[test]
fn test_class_file_top_level_property_getter_has_no_container() {
    let class_bytes = include_bytes!("resources/RememberKt.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse valid class file");

    let prop_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "getRememberFlag")
        .expect("Property getter should be parsed");
    assert_eq!(
        prop_decl.container_name, None,
        "Top-level property getter in *Kt should have no container name"
    );
}

#[test]
fn test_class_file_plain_java_class_retains_container() {
    let class_bytes = include_bytes!("resources/AndroidExternalSurfaceKt.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse class without kotlin metadata");

    let class_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "AndroidExternalSurfaceKt");
    assert!(
        class_decl.is_some(),
        "Plain Java class without @Metadata should be kept in declarations"
    );

    let method_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "AndroidExternalSurface")
        .expect("Method should be parsed");
    assert_eq!(
        method_decl.container_name.as_deref(),
        Some("AndroidExternalSurfaceKt"),
        "Static method in plain class should retain container name"
    );
}

#[test]
fn test_class_file_pure_java_class_retains_container() {
    let class_bytes = include_bytes!("resources/Calculator.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse valid Java class file");

    assert_eq!(
        file_symbols.package_name.as_deref(),
        Some("com.example.calc")
    );

    let class_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "Calculator");
    assert!(
        class_decl.is_some(),
        "Pure Java class Calculator should be present in declarations"
    );

    let add_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "add")
        .expect("Static method add should be parsed");
    assert_eq!(
        add_decl.container_name.as_deref(),
        Some("Calculator"),
        "Static Java method should retain class container name"
    );

    let multiply_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "multiply")
        .expect("Member method multiply should be parsed");
    assert_eq!(
        multiply_decl.container_name.as_deref(),
        Some("Calculator"),
        "Member Java method should retain class container name"
    );
}

#[test]
fn test_class_file_primitive_and_object_extension_methods() {
    let class_bytes = include_bytes!("resources/NumberExtensionsKt.class");
    let file_symbols =
        parse_class_file_symbols(class_bytes).expect("Should parse Kotlin file facade");

    assert_eq!(
        file_symbols.package_name.as_deref(),
        Some("com.example.math")
    );

    let facade_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "NumberExtensionsKt");
    assert!(
        facade_decl.is_none(),
        "File facade class should be dropped from declarations"
    );

    let is_even_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "isEven")
        .expect("Extension function isEven should be parsed");
    assert_eq!(
        is_even_decl.container_name.as_deref(),
        Some("kotlin.Int"),
        "Primitive receiver extension function should have container kotlin.Int"
    );
    assert_eq!(
        is_even_decl.signature.as_deref(),
        Some("fun Int.isEven(): Boolean"),
        "Primitive receiver extension function signature"
    );

    let shout_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "shout")
        .expect("Extension function shout should be parsed");
    assert_eq!(
        shout_decl.container_name.as_deref(),
        Some("java.lang.String"),
        "Object receiver extension function should have container java.lang.String"
    );
    assert_eq!(
        shout_decl.signature.as_deref(),
        Some("fun String.shout(): String"),
        "Object receiver extension function signature"
    );
}
