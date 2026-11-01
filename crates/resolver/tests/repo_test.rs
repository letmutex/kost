use resolver::{RepositoryRegistry, ScriptDialect};

#[test]
fn test_repositories_preserve_exact_gradle_declaration_order() {
    let mut registry = RepositoryRegistry::new();
    let kts_script = r#"
        repositories {
            mavenCentral()
            maven("https://mid.mirror.org/maven2")
            google()
            gradlePluginPortal()
        }
    "#;

    registry
        .parse_from_script(kts_script, ScriptDialect::Kotlin)
        .unwrap();

    let urls: Vec<&str> = registry
        .repositories
        .iter()
        .map(|repo| repo.url.as_str())
        .collect();

    assert_eq!(
        urls,
        vec![
            "https://repo1.maven.org/maven2/",
            "https://mid.mirror.org/maven2/",
            "https://maven.google.com/",
            "https://plugins.gradle.org/m2/",
        ]
    );
}

#[test]
fn test_settings_dependency_resolution_repos_precede_plugin_management_repos() {
    let mut registry = RepositoryRegistry::new();
    let settings_kts = r#"
        pluginManagement {
            repositories {
                gradlePluginPortal()
                google()
                mavenCentral()
            }
        }
        dependencyResolutionManagement {
            repositories {
                google()
                mavenCentral()
                maven("https://androidx.dev/storage/compose-compiler/repository")
            }
        }
    "#;

    registry
        .parse_from_script(settings_kts, ScriptDialect::Kotlin)
        .unwrap();

    let urls: Vec<&str> = registry
        .repositories
        .iter()
        .map(|repo| repo.url.as_str())
        .collect();

    assert_eq!(
        urls,
        vec![
            "https://maven.google.com/",
            "https://repo1.maven.org/maven2/",
            "https://androidx.dev/storage/compose-compiler/repository/",
            "https://plugins.gradle.org/m2/",
        ]
    );
}

#[test]
fn test_groovy_repo_map_literal_bare_label_and_bare_uri_shorthand() {
    let mut registry = RepositoryRegistry::new();
    let groovy_script = r#"
        repositories {
            maven([url: 'https://map-lit.example.com/repo'])
            maven url: 'https://bare-label.example.com/repo'
            maven uri('https://bare-uri.example.com/repo')
            jcenter()
        }
    "#;

    registry
        .parse_from_script(groovy_script, ScriptDialect::Groovy)
        .unwrap();

    let urls: Vec<&str> = registry
        .repositories
        .iter()
        .map(|repo| repo.url.as_str())
        .collect();

    assert_eq!(
        urls,
        vec![
            "https://map-lit.example.com/repo/",
            "https://bare-label.example.com/repo/",
            "https://bare-uri.example.com/repo/",
            "https://jcenter.bintray.com/",
        ]
    );
}

#[test]
fn test_calls_outside_repositories_block_are_ignored() {
    let mut registry = RepositoryRegistry::new();
    let kts_script = r#"
        maven("https://outside.example.com/repo")
        dependencies {
            implementation("com.example:lib:1.0.0")
        }
        repositories {
            maven("https://inside.example.com/repo")
        }
    "#;

    registry
        .parse_from_script(kts_script, ScriptDialect::Kotlin)
        .unwrap();

    let urls: Vec<&str> = registry
        .repositories
        .iter()
        .map(|repo| repo.url.as_str())
        .collect();

    assert_eq!(urls, vec!["https://inside.example.com/repo/"]);
}

#[test]
fn test_publishing_repositories_block_is_ignored() {
    let mut kt_registry = RepositoryRegistry::new();
    let kts_script = r#"
        publishing {
            repositories {
                maven {
                    url = uri("https://deploy.example.com/releases")
                }
            }
        }
        repositories {
            mavenCentral()
        }
    "#;
    kt_registry
        .parse_from_script(kts_script, ScriptDialect::Kotlin)
        .unwrap();
    assert_eq!(kt_registry.repositories.len(), 1);
    assert_eq!(
        kt_registry.repositories[0].url,
        "https://repo1.maven.org/maven2/"
    );

    let mut gr_registry = RepositoryRegistry::new();
    let groovy_script = r#"
        publishing {
            repositories {
                maven {
                    url 'https://deploy.example.com/groovy-releases'
                }
            }
        }
        repositories {
            google()
        }
    "#;
    gr_registry
        .parse_from_script(groovy_script, ScriptDialect::Groovy)
        .unwrap();
    assert_eq!(gr_registry.repositories.len(), 1);
    assert_eq!(gr_registry.repositories[0].url, "https://maven.google.com/");
}

#[test]
fn test_groovy_double_quoted_ext_script_detected_and_interpolated() {
    let groovy_script = r#"
ext {
    roomVersion = "2.6.1"
}
ext.okhttpVersion = "4.12.0"

dependencies {
    implementation("androidx.room:room-runtime:$roomVersion")
    implementation("com.squareup.okhttp3:okhttp:$okhttpVersion")
}
"#;
    assert_eq!(
        ScriptDialect::detect_from_content(groovy_script),
        ScriptDialect::Groovy
    );
    let info = resolver::BuildScriptInfo::parse(groovy_script, None);
    assert_eq!(info.dependencies.len(), 2);
    let room = info
        .dependencies
        .iter()
        .find(|dep| dep.name == "room-runtime")
        .unwrap();
    assert_eq!(room.version.as_ref().unwrap().raw(), "2.6.1");
}

#[test]
fn test_kotlin_chained_plugin_version_call_extracted() {
    let kts_script = r#"
plugins {
    id("org.jetbrains.kotlin.jvm").version("2.0.21").apply(false)
}
"#;
    let info = resolver::BuildScriptInfo::parse(kts_script, None);
    assert_eq!(info.kotlin_versions, vec!["2.0.21".to_string()]);
}

#[test]
fn test_sibling_submodules_inherit_root_vars_without_leaking_local_vars() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_resolver_sibling_isolation_",
        &nanos.to_string()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let mod_a = temp_dir.join("mod_a");
    let mod_b = temp_dir.join("mod_b");
    std::fs::create_dir_all(&mod_a).unwrap();
    std::fs::create_dir_all(&mod_b).unwrap();

    std::fs::write(temp_dir.join("gradle.properties"), "sharedVer=1.0.0\n").unwrap();
    std::fs::write(
        mod_a.join("build.gradle.kts"),
        "val sharedVer = \"9.9.9\"\ndependencies { implementation(\"com.example:lib-a:$sharedVer\") }\n",
    )
    .unwrap();
    std::fs::write(
        mod_b.join("build.gradle.kts"),
        "dependencies { implementation(\"com.example:lib-b:$sharedVer\") }\n",
    )
    .unwrap();

    let (catalogs, scripts) = resolver::discovery::discover_gradle_files(&temp_dir);
    let deps = resolver::discovery::collect_project_dependencies(&catalogs, &scripts).unwrap();
    let lib_a = deps.iter().find(|dep| dep.name == "lib-a").unwrap();
    let lib_b = deps.iter().find(|dep| dep.name == "lib-b").unwrap();
    assert_eq!(lib_a.version.as_ref().unwrap().raw(), "9.9.9");
    assert_eq!(lib_b.version.as_ref().unwrap().raw(), "1.0.0");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_transitive_graph_deduplicates_unsorted_direct_dependencies() {
    let direct = vec![
        resolver::DependencyCoordinate::new("com.example", "lib", Some("2.0.0")),
        resolver::DependencyCoordinate::new("com.example", "lib", Some("1.0.0")),
    ];
    let graph = resolver::DependencyGraph::resolve_transitive_graph(&direct, None);
    assert_eq!(graph.resolved_dependencies.len(), 1);
    assert_eq!(
        graph.resolved_dependencies[0]
            .version
            .as_ref()
            .unwrap()
            .raw(),
        "2.0.0"
    );
}
