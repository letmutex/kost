use ahash::AHashMap;
use lsp::gradle::build_script::{BuildScriptInfo, parse_gradle_properties};
use lsp::gradle::catalog::VersionCatalog;

#[test]
fn test_kts_build_script_parsing() {
    let script = r#"
plugins {
    id("org.jetbrains.kotlin.android") version "2.0.20"
}

android {
    compileSdk = 35
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    api("com.squareup.okhttp3:okhttp:4.12.0")
    compileOnly("javax.inject:javax.inject:1")
    implementation(libs.androidx.compose.ui)
}
"#;

    let toml = r#"
[libraries]
androidx-compose-ui = "androidx.compose.ui:ui:1.7.0"
"#;
    let catalog = VersionCatalog::parse(toml).unwrap();
    let info = BuildScriptInfo::parse(script, Some(&catalog));

    assert_eq!(info.compile_sdk, Some(35));
    assert!(info.kotlin_versions.contains(&"2.0.20".to_string()));
    assert_eq!(info.dependencies.len(), 4);

    let okhttp = info
        .dependencies
        .iter()
        .find(|d| d.name == "okhttp")
        .unwrap();
    assert_eq!(okhttp.version.as_ref().unwrap().raw(), "4.12.0");
}

#[test]
fn test_groovy_build_script_parsing() {
    let script = r#"
android {
    compileSdkVersion 34
}

dependencies {
    implementation 'androidx.appcompat:appcompat:1.7.0'
    testImplementation 'junit:junit:4.13.2'
}
"#;

    let info = BuildScriptInfo::parse(script, None);
    assert_eq!(info.compile_sdk, Some(34));
    assert_eq!(info.dependencies.len(), 2);
}

#[test]
fn test_kts_variable_resolution_val_and_interpolation() {
    let script = r#"
val roomVersion = "2.6.1"
val coroutines_version = "1.9.0"

dependencies {
    implementation("androidx.room:room-runtime:$roomVersion")
    implementation("androidx.room:room-ktx:${roomVersion}")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:" + coroutines_version)
    implementation("androidx.room", "room-testing", roomVersion)
}
"#;

    let info = BuildScriptInfo::parse(script, None);
    assert_eq!(info.variables.get("roomVersion").unwrap(), "2.6.1");
    assert_eq!(info.variables.get("coroutines_version").unwrap(), "1.9.0");

    let runtime = info
        .dependencies
        .iter()
        .find(|d| d.name == "room-runtime")
        .unwrap();
    assert_eq!(runtime.version.as_ref().unwrap().raw(), "2.6.1");

    let ktx = info
        .dependencies
        .iter()
        .find(|d| d.name == "room-ktx")
        .unwrap();
    assert_eq!(ktx.version.as_ref().unwrap().raw(), "2.6.1");

    let coroutines = info
        .dependencies
        .iter()
        .find(|d| d.name == "kotlinx-coroutines-core")
        .unwrap();
    assert_eq!(coroutines.version.as_ref().unwrap().raw(), "1.9.0");

    let testing = info
        .dependencies
        .iter()
        .find(|d| d.name == "room-testing")
        .unwrap();
    assert_eq!(testing.version.as_ref().unwrap().raw(), "2.6.1");
}

#[test]
fn test_groovy_variable_resolution_def_and_ext() {
    let script = r#"
def room_version = "2.6.1"
ext.lifecycle_version = '2.8.0'

dependencies {
    implementation "androidx.room:room-runtime:$room_version"
    implementation "androidx.room:room-ktx:${room_version}"
    implementation group: 'androidx.lifecycle', name: 'lifecycle-runtime', version: lifecycle_version
}
"#;

    let info = BuildScriptInfo::parse(script, None);
    assert_eq!(info.variables.get("room_version").unwrap(), "2.6.1");
    assert_eq!(info.variables.get("lifecycle_version").unwrap(), "2.8.0");

    let runtime = info
        .dependencies
        .iter()
        .find(|d| d.name == "room-runtime")
        .unwrap();
    assert_eq!(runtime.version.as_ref().unwrap().raw(), "2.6.1");

    let lifecycle = info
        .dependencies
        .iter()
        .find(|d| d.name == "lifecycle-runtime")
        .unwrap();
    assert_eq!(lifecycle.version.as_ref().unwrap().raw(), "2.8.0");
}

#[test]
fn test_gradle_properties_and_inherited_variables() {
    let props = r#"
# Project versions
roomVersion=2.6.1
kotlin.version=2.0.20
"#;
    let mut vars = AHashMap::default();
    parse_gradle_properties(props, &mut vars);
    assert_eq!(vars.get("roomVersion").unwrap(), "2.6.1");

    let script = r#"
dependencies {
    implementation("androidx.room:room-runtime:$roomVersion")
}
"#;
    let info = BuildScriptInfo::parse_with_vars(script, None, &vars);
    let runtime = info
        .dependencies
        .iter()
        .find(|d| d.name == "room-runtime")
        .unwrap();
    assert_eq!(runtime.version.as_ref().unwrap().raw(), "2.6.1");
}

#[test]
fn test_android_block_and_compile_sdk_parsing() {
    // Both android {} and compileSdk present
    let script_kts = r#"
android {
    namespace = "com.example.app"
    compileSdk = 35
}
"#;
    let info_kts = BuildScriptInfo::parse(script_kts, None);
    assert!(info_kts.has_android_block);
    assert_eq!(info_kts.compile_sdk, Some(35));

    // android {} present but no compileSdk
    let script_no_sdk = r#"
android {
    namespace = "com.example.app"
}
"#;
    let info_no_sdk = BuildScriptInfo::parse(script_no_sdk, None);
    assert!(info_no_sdk.has_android_block);
    assert_eq!(info_no_sdk.compile_sdk, None);

    // compileSdk with variable inside android {}
    let script_var = r#"
val mySdk = "34"
android {
    compileSdk = mySdk
}
"#;
    let info_var = BuildScriptInfo::parse(script_var, None);
    assert!(info_var.has_android_block);
    assert_eq!(info_var.compile_sdk, Some(34));
}

#[test]
fn test_compile_sdk_without_android_block_ignored() {
    let script_pure_jvm = r#"
plugins {
    kotlin("jvm") version "2.0.20"
}
// compileSdk = 35 outside android block
val compileSdk = 35
dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
}
"#;
    let info = BuildScriptInfo::parse(script_pure_jvm, None);
    assert!(!info.has_android_block);
    assert_eq!(info.compile_sdk, None);
}

#[test]
fn test_collect_declared_dependencies_selects_highest_android_compile_sdk() {
    let fixtures_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("gradle")
        .join("fixtures");
    let script_34 = fixtures_dir.join("project/submodule/build.gradle");
    let script_35 = fixtures_dir.join("project/build.gradle.kts");

    // Pass script_34 first to ensure it selects the highest SDK rather than first-encountered
    let (_, _, pref_sdk) =
        lsp::gradle::discovery::collect_declared_dependencies(&[], &[script_34, script_35]);

    assert_eq!(pref_sdk, Some(35));
}
