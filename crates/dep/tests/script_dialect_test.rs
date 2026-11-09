#[test]
fn syntax_error_returns_parse_error() {
    let temp_dir = std::env::temp_dir().join("kost_test_syntax_error");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let kts_file = temp_dir.join("build.gradle.kts");
    std::fs::write(&kts_file, "dependencies { implementation( }").unwrap();
    let result = dep::discovery::collect_project_dependencies(&[], &[kts_file]);
    assert!(matches!(result, Err(dep::ParseError::SyntaxError(_))));

    let groovy_file = temp_dir.join("build.gradle");
    std::fs::write(&groovy_file, "dependencies { implementation( }").unwrap();
    let result = dep::discovery::collect_project_dependencies(&[], &[groovy_file]);
    assert!(matches!(result, Err(dep::ParseError::SyntaxError(_))));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn unknown_dialect_returns_parse_error() {
    let temp_dir = std::env::temp_dir().join("kost_test_unknown_dialect");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let unknown_file = temp_dir.join("build.gradle.unknown");
    std::fs::write(&unknown_file, "some content").unwrap();
    let result = dep::discovery::collect_project_dependencies(&[], &[unknown_file]);
    assert!(matches!(result, Err(dep::ParseError::UnknownDialect(_))));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn groovy_repo_uri_wrapper() {
    let mut registry = dep::RepositoryRegistry::new();
    let groovy_script = r#"
        repositories {
            maven {
                url = uri('https://uri-assigned.example.com/repo')
            }
            maven {
                url uri('https://uri-command.example.com/repo')
            }
            maven(uri('https://uri-call.example.com/repo'))
        }
    "#;

    registry
        .parse_from_script(groovy_script, dep::ScriptDialect::Groovy)
        .unwrap();

    let urls: Vec<&str> = registry
        .repositories
        .iter()
        .map(|repo| repo.url.as_str())
        .collect();

    assert!(urls.contains(&"https://uri-assigned.example.com/repo/"));
    assert!(urls.contains(&"https://uri-command.example.com/repo/"));
    assert!(urls.contains(&"https://uri-call.example.com/repo/"));
}

#[test]
fn groovy_ext_block_properties() {
    let mut deps = Vec::new();
    resolver::ast::groovy::parse_groovy_script(
        r#"
        ext {
            ktorVersion = "2.3.12"
            composeVer = "1.6.0"
        }
        dependencies {
            implementation "io.ktor:ktor-client-core:$ktorVersion"
            implementation "androidx.compose.ui:ui:$composeVer"
        }
        "#,
        &ahash::AHashMap::default(),
        &mut deps,
    )
    .unwrap();
    assert_eq!(deps.len(), 2);

    let ktor = deps
        .iter()
        .find(|dep| dep.name == "ktor-client-core")
        .unwrap();
    assert_eq!(ktor.version.as_ref().unwrap().raw(), "2.3.12");

    let compose = deps.iter().find(|dep| dep.name == "ui").unwrap();
    assert_eq!(compose.version.as_ref().unwrap().raw(), "1.6.0");
}

#[test]
fn groovy_dependencies_closure_argument_list() {
    let mut deps = Vec::new();
    resolver::ast::groovy::parse_groovy_script(
        r#"
        dependencies({
            implementation 'org.example:wrapped-in-call:1.0.0'
        })
        "#,
        &ahash::AHashMap::default(),
        &mut deps,
    )
    .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name.as_str(), "wrapped-in-call");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "1.0.0");
}

#[test]
fn string_escape_sequences_in_kotlin_and_groovy() {
    let mut kt_props = ahash::AHashMap::default();
    kt_props.insert("notInterpolated".to_string(), "WRONG".to_string());
    let mut kt_deps = Vec::new();
    resolver::ast::kotlin::parse_kotlin_script(
        r#"
        val ver = "1.0.0"
        dependencies {
            implementation("org.example:my-escaped-\$notInterpolated:$ver")
        }
        "#,
        &kt_props,
        &mut kt_deps,
    )
    .unwrap();
    assert_eq!(kt_deps.len(), 1);
    assert_eq!(kt_deps[0].name.as_str(), "my-escaped-$notInterpolated");
    assert_eq!(kt_deps[0].version.as_ref().unwrap().raw(), "1.0.0");

    let mut gr_deps = Vec::new();
    resolver::ast::groovy::parse_groovy_script(
        r#"
        def ver = "2.0.0"
        dependencies {
            implementation "org.example:groovy-escaped-\$notInterpolated:$ver"
        }
        "#,
        &kt_props,
        &mut gr_deps,
    )
    .unwrap();
    assert_eq!(gr_deps.len(), 1);
    assert_eq!(gr_deps[0].name.as_str(), "groovy-escaped-$notInterpolated");
    assert_eq!(gr_deps[0].version.as_ref().unwrap().raw(), "2.0.0");
}

#[test]
fn configuration_matching_camel_case_api() {
    let mut deps = Vec::new();
    resolver::ast::kotlin::parse_kotlin_script(
        r#"
        dependencies {
            commonMainApi("org.example:kmp-api:1.0.0")
            // openapi is not a valid dependency configuration and should be ignored
            openapi("org.example:ignored-openapi:1.0.0")
        }
        "#,
        &ahash::AHashMap::default(),
        &mut deps,
    )
    .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name.as_str(), "kmp-api");
}

#[test]
fn groovy_command_chain_platform_bom() {
    let mut deps = Vec::new();
    resolver::ast::groovy::parse_groovy_script(
        r#"
        dependencies {
            implementation platform 'org.example.bom:bom-core:1.2.3'
        }
        "#,
        &ahash::AHashMap::default(),
        &mut deps,
    )
    .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name.as_str(), "bom-core");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "1.2.3");
}

#[test]
fn named_arguments_with_variable_and_three_positional_args() {
    let mut kt_deps = Vec::new();
    resolver::ast::kotlin::parse_kotlin_script(
        r#"
        val ktVer = "3.2.1"
        val fullCoord = "org.example:var-coord:4.0.0"
        dependencies {
            implementation(group = "org.example", name = "named-kt", version = ktVer)
            implementation("org.example", "pos-kt", ktVer)
            implementation(fullCoord)
            implementation("org.example:concat-kt:" + ktVer)
        }
        "#,
        &ahash::AHashMap::default(),
        &mut kt_deps,
    )
    .unwrap();
    assert_eq!(kt_deps.len(), 4);
    for expected in ["named-kt", "pos-kt", "concat-kt"] {
        assert!(
            kt_deps
                .iter()
                .any(|dep| dep.name == expected && dep.version.as_ref().unwrap().raw() == "3.2.1")
        );
    }
    assert!(
        kt_deps
            .iter()
            .any(|dep| dep.name == "var-coord" && dep.version.as_ref().unwrap().raw() == "4.0.0")
    );

    let mut gr_deps = Vec::new();
    resolver::ast::groovy::parse_groovy_script(
        r#"
        def grVer = "2.4.0"
        def fullGrCoord = "org.example:var-gr:5.0.0"
        dependencies {
            implementation(group: 'org.example', name: 'named-gr', version: grVer)
            implementation([group: 'org.example', name: 'map-gr', version: grVer])
            implementation('org.example', 'pos-gr', grVer)
            implementation(fullGrCoord)
        }
        "#,
        &ahash::AHashMap::default(),
        &mut gr_deps,
    )
    .unwrap();
    assert_eq!(gr_deps.len(), 4);
    for expected in ["named-gr", "map-gr", "pos-gr"] {
        assert!(
            gr_deps
                .iter()
                .any(|dep| dep.name == expected && dep.version.as_ref().unwrap().raw() == "2.4.0")
        );
    }
    assert!(
        gr_deps
            .iter()
            .any(|dep| dep.name == "var-gr" && dep.version.as_ref().unwrap().raw() == "5.0.0")
    );
}

#[test]
fn nested_buildscript_and_delegate_properties() {
    let mut kt_deps = Vec::new();
    resolver::ast::kotlin::parse_kotlin_script(
        r#"
        val roomVersion by extra("2.6.1")
        dependencies {
            val localKaptVer = "2.51.1"
            kapt("androidx.room:room-compiler:$roomVersion")
            testAnnotationProcessor("com.google.dagger:dagger-compiler:$localKaptVer")
        }
        "#,
        &ahash::AHashMap::default(),
        &mut kt_deps,
    )
    .unwrap();
    assert_eq!(kt_deps.len(), 2);
    assert!(
        kt_deps.iter().any(
            |dep| dep.name == "room-compiler" && dep.version.as_ref().unwrap().raw() == "2.6.1"
        )
    );
    assert!(kt_deps.iter().any(
        |dep| dep.name == "dagger-compiler" && dep.version.as_ref().unwrap().raw() == "2.51.1"
    ));

    let mut gr_deps = Vec::new();
    resolver::ast::groovy::parse_groovy_script(
        r#"
        buildscript {
            ext {
                kotlinVersion = "1.9.22"
            }
            dependencies {
                classpath "org.jetbrains.kotlin:kotlin-gradle-plugin:${ext.kotlinVersion}"
            }
        }
        "#,
        &ahash::AHashMap::default(),
        &mut gr_deps,
    )
    .unwrap();
    assert_eq!(gr_deps.len(), 1);
    assert_eq!(gr_deps[0].name.as_str(), "kotlin-gradle-plugin");
    assert_eq!(gr_deps[0].version.as_ref().unwrap().raw(), "1.9.22");
}

#[test]
fn repository_property_interpolation() {
    let mut kt_registry = dep::RepositoryRegistry::new();
    let kts_script = r#"
        val customUrl = "https://custom-kotlin.example.com/repo"
        val namedUrl = "https://named-kotlin.example.com/repo"
        repositories {
            maven(customUrl)
            maven(url = namedUrl)
        }
    "#;
    kt_registry
        .parse_from_script(kts_script, dep::ScriptDialect::Kotlin)
        .unwrap();
    assert!(
        kt_registry
            .repositories
            .iter()
            .any(|repo| repo.url == "https://custom-kotlin.example.com/repo/")
    );
    assert!(
        kt_registry
            .repositories
            .iter()
            .any(|repo| repo.url == "https://named-kotlin.example.com/repo/")
    );

    let mut gr_registry = dep::RepositoryRegistry::new();
    let groovy_script = r#"
        def customUrl = "https://custom-groovy.example.com/repo"
        def namedUrl = "https://named-groovy.example.com/repo"
        repositories {
            maven { url = "$customUrl" }
            maven(url: namedUrl)
        }
    "#;
    gr_registry
        .parse_from_script(groovy_script, dep::ScriptDialect::Groovy)
        .unwrap();
    assert!(
        gr_registry
            .repositories
            .iter()
            .any(|repo| repo.url == "https://custom-groovy.example.com/repo/")
    );
    assert!(
        gr_registry
            .repositories
            .iter()
            .any(|repo| repo.url == "https://named-groovy.example.com/repo/")
    );
}
