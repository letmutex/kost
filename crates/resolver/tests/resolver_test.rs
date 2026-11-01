use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use resolver::{
    BuildScriptInfo, DependencyCoordinate, DependencyGraph, GradleCache, ModuleDescriptor,
    ScriptDialect, VersionCatalog,
};

#[test]
fn test_ast_build_script_resolves_catalog_bundles_and_plugin_aliases() {
    let catalog_toml = r#"
[versions]
kotlin = "2.0.21"
compose = "1.7.0"

[libraries]
compose-ui = { group = "androidx.compose.ui", name = "ui", version.ref = "compose" }
compose-material3 = { module = "androidx.compose.material3:material3", version.ref = "compose" }

[bundles]
compose = ["compose-ui", "compose-material3"]

[plugins]
kotlin-android = { id = "org.jetbrains.kotlin.android", version.ref = "kotlin" }
"#;
    let catalog = VersionCatalog::parse(catalog_toml).unwrap();

    let kts_script = r#"
plugins {
    alias(libs.plugins.kotlin.android)
}

extra["customVer"] = "3.1.0"

android {
    compileSdk(35)
}

dependencies {
    implementation(libs.bundles.compose)
    implementation("com.example:extra-lib:${extra["customVer"]}")
    implementation("com.example:direct-var:$customVer")
}
"#;

    let info = BuildScriptInfo::parse_with_dialect(
        kts_script,
        ScriptDialect::Kotlin,
        Some(&catalog),
        &ahash::AHashMap::default(),
    )
    .unwrap();

    assert!(info.has_android_block);
    assert_eq!(info.compile_sdk, Some(35));
    assert_eq!(info.kotlin_versions, vec!["2.0.21".to_string()]);
    assert_eq!(
        info.variables.get("customVer").map(String::as_str),
        Some("3.1.0")
    );
    assert!(info.dependencies.iter().any(|dep| dep.name == "ui"));
    assert!(info.dependencies.iter().any(|dep| dep.name == "material3"));
    assert!(
        info.dependencies
            .iter()
            .any(|dep| dep.name == "extra-lib" && dep.version.as_ref().unwrap().raw() == "3.1.0")
    );
    assert!(
        info.dependencies
            .iter()
            .any(|dep| dep.name == "direct-var" && dep.version.as_ref().unwrap().raw() == "3.1.0")
    );
}

#[test]
fn test_dialect_detection_groovy_double_quoted_labeled_map_and_annotated_kotlin_val() {
    let groovy_script = r#"
dependencies {
    implementation group: "androidx.lifecycle", name: "lifecycle-runtime", version: "2.8.0"
}
"#;
    assert_eq!(
        ScriptDialect::detect_from_content(groovy_script),
        ScriptDialect::Groovy
    );
    let info = BuildScriptInfo::parse(groovy_script, None);
    assert_eq!(info.dependencies.len(), 1);
    assert_eq!(info.dependencies[0].name, "lifecycle-runtime");
    assert_eq!(
        info.dependencies[0].version.as_ref().unwrap().raw(),
        "2.8.0"
    );

    let annotated_kts = r#"
@Suppress("unused") val roomVersion = "2.6.1"
dependencies {
    implementation(
}
"#;
    assert_eq!(
        ScriptDialect::detect_from_content(annotated_kts),
        ScriptDialect::Kotlin
    );
}

#[test]
fn test_groovy_closure_split_statement_resolves_ext_property_reference() {
    let groovy_script = r#"
ext.okhttpCoord = "com.squareup.okhttp3:okhttp:4.12.0"
dependencies {
    implementation rootProject.ext.okhttpCoord
}
"#;
    let info = BuildScriptInfo::parse(groovy_script, None);
    assert_eq!(info.dependencies.len(), 1);
    assert_eq!(info.dependencies[0].group, "com.squareup.okhttp3");
    assert_eq!(info.dependencies[0].name, "okhttp");
    assert_eq!(
        info.dependencies[0].version.as_ref().unwrap().raw(),
        "4.12.0"
    );
}

#[test]
fn test_discover_gradle_files_orders_root_script_before_submodule() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_resolver_submodule_order_",
        &nanos.to_string()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    let app_dir = temp_dir.join("app");
    fs::create_dir_all(&app_dir).unwrap();

    fs::write(
        temp_dir.join("build.gradle"),
        "ext { roomVersion = '2.6.1' }\n",
    )
    .unwrap();
    fs::write(
        app_dir.join("build.gradle"),
        "dependencies { implementation \"androidx.room:room-runtime:$roomVersion\" }\n",
    )
    .unwrap();

    let (catalogs, scripts) = resolver::discovery::discover_gradle_files(&temp_dir);
    let deps = resolver::discovery::collect_project_dependencies(&catalogs, &scripts).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "room-runtime");
    assert_eq!(deps[0].version.as_ref().map(|ver| ver.raw()), Some("2.6.1"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_transitive_graph_kmp_module_redirect_promotes_without_duplicating() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_resolver_kmp_redirect_",
        &nanos.to_string()
    ));
    let _ = fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).unwrap();
    let root_coord = DependencyCoordinate::new("org.example", "kmp-lib", Some("1.5.0"));
    let module_json = r#"{
        "formatVersion": "1.1",
        "component": {
            "group": "org.example",
            "module": "kmp-lib",
            "version": "1.5.0"
        },
        "variants": [
            {
                "name": "jvmRuntimeElements-published",
                "available-at": {
                    "url": "../../kmp-lib-jvm/1.5.0/kmp-lib-jvm-1.5.0.module",
                    "group": "org.example",
                    "module": "kmp-lib-jvm",
                    "version": "1.5.0"
                }
            }
        ]
    }"#;
    cache
        .store_file(&root_coord, "kmp-lib-1.5.0.module", module_json.as_bytes())
        .unwrap();

    let graph = DependencyGraph::resolve_transitive_graph(&[root_coord], Some(&temp_dir));
    assert_eq!(graph.direct_count, 1);
    assert_eq!(graph.transitive_count, 0);
    assert_eq!(graph.resolved_dependencies.len(), 1);
    assert_eq!(graph.resolved_dependencies[0].name, "kmp-lib-jvm");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_kotlin_non_block_android_call_does_not_set_has_android_block() {
    let script = r#"
plugins {
    kotlin("jvm") version "2.0.20"
}
android("custom-arg")
"#;
    let info = BuildScriptInfo::parse(script, None);
    assert!(!info.has_android_block);
}

#[test]
fn test_groovy_ast_build_script_plugins_and_catalog_accessors() {
    let catalog_toml = r#"
[versions]
kotlin = "1.9.24"
okhttp = "4.12.0"

[libraries]
okhttp-core = { module = "com.squareup.okhttp3:okhttp", version.ref = "okhttp" }

[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version.ref = "kotlin" }
"#;
    let catalog = VersionCatalog::parse(catalog_toml).unwrap();

    let groovy_script = r#"
plugins {
    id 'org.jetbrains.kotlin.android' version '2.0.10'
    alias libs.plugins.kotlin.jvm
}

android {
    compileSdk = 34
}

dependencies {
    implementation libs.okhttp.core
}
"#;

    let info = BuildScriptInfo::parse_with_dialect(
        groovy_script,
        ScriptDialect::Groovy,
        Some(&catalog),
        &ahash::AHashMap::default(),
    )
    .unwrap();

    assert!(info.has_android_block);
    assert_eq!(info.compile_sdk, Some(34));
    assert!(info.kotlin_versions.contains(&"2.0.10".to_string()));
    assert!(info.kotlin_versions.contains(&"1.9.24".to_string()));
    assert_eq!(info.dependencies.len(), 1);
    assert_eq!(info.dependencies[0].name, "okhttp");
    assert_eq!(
        info.dependencies[0].version.as_ref().unwrap().raw(),
        "4.12.0"
    );
}

#[test]
fn test_cached_transitive_graph_combines_descriptors_and_cached_poms() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_resolver_hybrid_graph_",
        &nanos.to_string()
    ));
    let _ = fs::remove_dir_all(&temp_dir);

    let desc_dir =
        temp_dir.join("caches/modules-2/metadata-2.107/descriptors/com.example/app/1.0.0/hash1");
    fs::create_dir_all(&desc_dir).unwrap();
    let desc_bytes = ModuleDescriptor::create_test_descriptor_bytes(
        "com.example",
        "app",
        "1.0.0",
        &[("com.example", "mid", "1.0.0")],
        &["app-1.0.0.jar"],
    );
    fs::write(desc_dir.join("descriptor.bin"), desc_bytes).unwrap();

    let cache = GradleCache::new(Some(&temp_dir)).unwrap();
    let mid_coord = DependencyCoordinate::new("com.example", "mid", Some("1.0.0"));
    let mid_pom = r#"<project>
        <groupId>com.example</groupId>
        <artifactId>mid</artifactId>
        <version>1.0.0</version>
        <dependencies>
            <dependency>
                <groupId>com.example</groupId>
                <artifactId>leaf</artifactId>
                <version>2.3.0</version>
            </dependency>
        </dependencies>
    </project>"#;
    cache
        .store_file(&mid_coord, "mid-1.0.0.pom", mid_pom.as_bytes())
        .unwrap();

    let direct = vec![DependencyCoordinate::new(
        "com.example",
        "app",
        Some("1.0.0"),
    )];
    let graph = DependencyGraph::resolve_transitive_graph(&direct, Some(&temp_dir));

    assert_eq!(graph.direct_count, 1);
    assert_eq!(graph.transitive_count, 2);
    assert_eq!(graph.descriptors_parsed, 1);
    assert!(
        graph
            .resolved_dependencies
            .iter()
            .any(|dep| dep.name == "leaf" && dep.version.as_ref().unwrap().raw() == "2.3.0")
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
