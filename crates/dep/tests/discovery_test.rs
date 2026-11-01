#[test]
fn test_catalog_dependencies_parsing() {
    let toml = r#"
[versions]
coroutines-ver = "1.8.0"

[libraries]
compose-ui = "androidx.compose.ui:ui:1.6.0"
coroutines = { module = "org.jetbrains.kotlinx:kotlinx-coroutines-core", version.ref = "coroutines-ver" }
okhttp = { group = "com.squareup.okhttp3", name = "okhttp", version = "4.12.0" }
"#;
    let temp_dir = std::env::temp_dir().join("kost_test_catalog_parsing");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);
    let catalog_file = temp_dir.join("libs.versions.toml");
    std::fs::write(&catalog_file, toml).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[catalog_file], &[]);
    assert_eq!(deps.len(), 3);
    assert!(
        deps.iter()
            .any(|d| d.name == "ui" && d.group == "androidx.compose.ui")
    );

    let coroutines_dep = deps
        .iter()
        .find(|d| d.name == "kotlinx-coroutines-core")
        .unwrap();
    assert_eq!(coroutines_dep.version.as_ref().unwrap().raw(), "1.8.0");

    let okhttp_dep = deps.iter().find(|d| d.name == "okhttp").unwrap();
    assert_eq!(okhttp_dep.group, "com.squareup.okhttp3");
    assert_eq!(okhttp_dep.version.as_ref().unwrap().raw(), "4.12.0");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_script_dependencies_with_android_and_kmp_configurations() {
    let temp_dir = std::env::temp_dir().join("kost_test_script_kmp_configs");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);
    let script_file = temp_dir.join("build.gradle.kts");
    let script = r#"
        dependencies {
            debugImplementation("com.squareup.leakcanary:leakcanary-android:2.14")
            commonMainImplementation("io.ktor:ktor-client-core:2.3.12")
            ksp 'com.google.dagger:hilt-compiler:2.51'
            annotationProcessor "com.google.auto.value:auto-value:1.10.4"
        }
    "#;
    std::fs::write(&script_file, script).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[], &[script_file]);
    assert_eq!(deps.len(), 4);
    assert!(deps.iter().any(|d| d.name == "leakcanary-android"));
    assert!(deps.iter().any(|d| d.name == "ktor-client-core"));
    assert!(deps.iter().any(|d| d.name == "hilt-compiler"));
    assert!(deps.iter().any(|d| d.name == "auto-value"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_script_dependencies_comment_stripping() {
    let temp_dir = std::env::temp_dir().join("kost_test_script_comments");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);
    let script_file = temp_dir.join("build.gradle.kts");
    let script = r#"
        // implementation("com.example:commented-line:1.0")
        /*
         * implementation("com.example:commented-block:1.0")
         */
        implementation("com.example:active:1.0") // trailing comment
        maven("https://repo.example.com/maven2")
        /* implementation("com.example:inline-block:1.0") */
        ksp("com.example:active-ksp:2.0")
    "#;
    std::fs::write(&script_file, script).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[], &[script_file]);
    assert_eq!(deps.len(), 2);
    assert!(deps.iter().any(|d| d.name == "active"));
    assert!(deps.iter().any(|d| d.name == "active-ksp"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_script_dependencies_property_interpolation() {
    let temp_dir = std::env::temp_dir().join("kost_test_script_props");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let props_file = temp_dir.join("gradle.properties");
    std::fs::write(&props_file, "coroutinesVersion=1.9.0\nasm_ver=9.6\n").unwrap();

    let script_file = temp_dir.join("build.gradle.kts");
    let script = r#"
        implementation "org.jetbrains.kotlinx:kotlinx-coroutines-core:$coroutinesVersion"
        implementation "org.ow2.asm:asm:${asm_ver}"
    "#;
    std::fs::write(&script_file, script).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[], &[props_file, script_file]);
    assert_eq!(deps.len(), 2);
    let coroutines = deps
        .iter()
        .find(|d| d.name == "kotlinx-coroutines-core")
        .unwrap();
    assert_eq!(coroutines.version.as_ref().unwrap().raw(), "1.9.0");
    let asm = deps.iter().find(|d| d.name == "asm").unwrap();
    assert_eq!(asm.version.as_ref().unwrap().raw(), "9.6");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_script_dependencies_platform_bom_and_named_arguments() {
    let temp_dir = std::env::temp_dir().join("kost_test_script_bom");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let script_file = temp_dir.join("build.gradle.kts");
    let script = r#"
        dependencies {
            implementation(platform("org.jetbrains.kotlin:kotlin-bom:2.0.0"))
            api(enforcedPlatform("com.google.cloud:libraries-bom:26.37.0"))
            implementation(group = "org.example", name = "custom-named", version = "3.2.1")
        }
    "#;
    std::fs::write(&script_file, script).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[], &[script_file]);
    assert_eq!(deps.len(), 3);
    assert!(
        deps.iter()
            .any(|d| d.name == "kotlin-bom" && d.version.as_ref().unwrap().raw() == "2.0.0")
    );
    assert!(
        deps.iter()
            .any(|d| d.name == "libraries-bom" && d.version.as_ref().unwrap().raw() == "26.37.0")
    );
    assert!(
        deps.iter()
            .any(|d| d.name == "custom-named" && d.version.as_ref().unwrap().raw() == "3.2.1")
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_script_variable_prefix_collision_and_local_val_interpolation() {
    let temp_dir = std::env::temp_dir().join("kost_test_script_var_prefix");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let props_file = temp_dir.join("gradle.properties");
    std::fs::write(&props_file, "kotlin=1.9.0\nkotlinVersion=2.0.20\n").unwrap();

    let script_file = temp_dir.join("build.gradle.kts");
    let script = r#"
        val ktorVersion = "2.3.12"
        val escapedBackslash = "C:\\" // comment after escaped backslash
        dependencies {
            implementation("org.jetbrains.kotlin:kotlin-stdlib:$kotlinVersion")
            implementation("io.ktor:ktor-client-core:$ktorVersion")
        }
    "#;
    std::fs::write(&script_file, script).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[], &[props_file, script_file]);
    assert_eq!(deps.len(), 2);

    let stdlib = deps.iter().find(|d| d.name == "kotlin-stdlib").unwrap();
    assert_eq!(stdlib.version.as_ref().unwrap().raw(), "2.0.20");

    let ktor = deps.iter().find(|d| d.name == "ktor-client-core").unwrap();
    assert_eq!(ktor.version.as_ref().unwrap().raw(), "2.3.12");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
