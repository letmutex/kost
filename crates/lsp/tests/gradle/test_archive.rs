use std::path::PathBuf;

use lsp::gradle::archive::ArchiveIndexer;

#[test]
fn test_index_sources_jar_in_memory() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("gradle")
        .join("fixtures");
    let jar_path = fixtures_dir.join(
        "gradle_cache/caches/modules-2/files-2.1/androidx.core/core-ktx/1.15.0/6677889900/core-ktx-1.15.0-sources.jar",
    );

    let symbols = ArchiveIndexer::index_archive(&jar_path, true);
    assert!(!symbols.is_empty());

    let (_, file_sym) = &symbols[0];
    assert_eq!(file_sym.package_name.as_deref(), Some("androidx.core.view"));
    assert!(
        file_sym
            .declarations
            .iter()
            .any(|d| d.name == "isNewKtxVersion")
    );
    assert!(file_sym.declarations.iter().any(|d| d.name == "isVisible"));
}

#[test]
fn test_index_aar_in_memory() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("gradle")
        .join("fixtures");
    let aar_path = fixtures_dir.join(
        "gradle_cache/caches/modules-2/files-2.1/androidx.core/core-ktx/1.15.0/1122334455/core-ktx-1.15.0.aar",
    );

    let symbols = ArchiveIndexer::index_archive(&aar_path, false);
    assert!(!symbols.is_empty());

    let (_, file_sym) = &symbols[0];
    assert_eq!(file_sym.package_name.as_deref(), Some("androidx.core.view"));
    assert!(file_sym.declarations.iter().any(|d| d.name == "ViewCompat"));
}

#[test]
fn test_parse_class_file_extension_methods_and_mangled_filtering() {
    let class_bytes = include_bytes!("resources/AndroidExternalSurfaceKt.class");
    let file_symbols = lsp::gradle::class_file::parse_class_file_symbols(class_bytes)
        .expect("Should parse valid class file");

    assert_eq!(
        file_symbols.package_name.as_deref(),
        Some("androidx.compose.foundation")
    );

    let decl_names: Vec<String> = file_symbols
        .declarations
        .iter()
        .map(|d| d.name.to_string())
        .collect();
    assert!(decl_names.contains(&"padding".to_string()));
    assert!(decl_names.contains(&"AndroidExternalSurface".to_string()));
    assert!(!decl_names.contains(&"AndroidExternalSurface-58FFMhA".to_string()));
    assert!(!decl_names.contains(&"AndroidExternalSurface$default".to_string()));

    let padding_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "padding")
        .unwrap();
    assert_eq!(
        padding_decl.container_name.as_deref(),
        Some("androidx.compose.ui.Modifier")
    );

    let surface_decl = file_symbols
        .declarations
        .iter()
        .find(|d| d.name == "AndroidExternalSurface")
        .unwrap();
    assert_eq!(
        surface_decl.container_name.as_deref(),
        Some("AndroidExternalSurfaceKt")
    );
}
