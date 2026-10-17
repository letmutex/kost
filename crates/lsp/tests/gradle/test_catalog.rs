use lsp::gradle::catalog::VersionCatalog;

#[test]
fn test_catalog_parsing_and_resolution() {
    let toml = r#"
[versions]
kotlin = "2.0.20"
coreKtx = "1.15.0"
compose = "1.7.0"

[libraries]
androidx-core-ktx = { group = "androidx.core", name = "core-ktx", version.ref = "coreKtx" }
androidx-compose-ui = { module = "androidx.compose.ui:ui", version.ref = "compose" }
gson = "com.google.code.gson:gson:2.10.1"

[bundles]
ui = ["androidx-core-ktx", "androidx-compose-ui"]

[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version.ref = "kotlin" }
"#;

    let catalog = VersionCatalog::parse(toml).expect("Catalog parsing should succeed");

    assert_eq!(catalog.versions.get("coreKtx").unwrap(), "1.15.0");

    let core_ktx = catalog.resolve_accessor("libs.androidx.core.ktx");
    assert_eq!(core_ktx.len(), 1);
    assert_eq!(core_ktx[0].group, "androidx.core");
    assert_eq!(core_ktx[0].name, "core-ktx");
    assert_eq!(core_ktx[0].version.as_ref().unwrap().raw(), "1.15.0");

    let bundle = catalog.resolve_accessor("libs.bundles.ui");
    assert_eq!(bundle.len(), 2);

    let all_deps = catalog.all_dependencies();
    assert_eq!(all_deps.len(), 3);
}
