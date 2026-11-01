use resolver::{BuildScriptInfo, DependencyCoordinate, GradleCache, PomModel, ScriptDialect};

#[test]
fn test_groovy_double_quoted_command_chains_and_split_closure_expressions() {
    let groovy_script = r#"
roomVersion = "2.6.1"
mySdk = 34
kotlinVer = "2.0.20"
coordVar = "com.squareup.okhttp3:okhttp:4.12.0"
repoUrl = "https://custom.groovy.example.com/maven2"

plugins {
    id "org.jetbrains.kotlin.jvm" version kotlinVer
}

android {
    compileSdkVersion mySdk
}

repositories {
    maven {
        url repoUrl
    }
}

dependencies {
    implementation "androidx.room:room-runtime:$roomVersion"
    implementation platform("androidx.compose:compose-bom:2024.04.01")
    implementation "org.jetbrains.kotlinx:kotlinx-coroutines-core:" + "1.8.0"
    implementation coordVar
    implementation [group: "com.google.code.gson", name: "gson", version: "2.10.1"]
}
"#;

    assert_eq!(
        ScriptDialect::detect_from_content(groovy_script),
        ScriptDialect::Groovy
    );
    let info = BuildScriptInfo::parse(groovy_script, None);
    assert!(info.has_android_block);
    assert_eq!(info.compile_sdk, Some(34));
    assert_eq!(info.kotlin_versions, vec!["2.0.20".to_string()]);
    assert_eq!(
        info.repositories.urls,
        vec!["https://custom.groovy.example.com/maven2".to_string()]
    );
    assert_eq!(info.dependencies.len(), 5);
    assert!(
        info.dependencies
            .iter()
            .any(|dep| dep.name == "room-runtime")
    );
    assert!(
        info.dependencies
            .iter()
            .any(|dep| dep.name == "compose-bom")
    );
    assert!(
        info.dependencies
            .iter()
            .any(|dep| dep.name == "kotlinx-coroutines-core")
    );
    assert!(info.dependencies.iter().any(|dep| dep.name == "okhttp"));
    assert!(info.dependencies.iter().any(|dep| dep.name == "gson"));
}

#[test]
fn test_pom_child_property_override_applies_to_parent_dependency_management() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_resolver_pom_prop_override_",
        &nanos.to_string()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let cache = GradleCache::new(Some(&temp_dir)).unwrap();

    let parent_coord = DependencyCoordinate::new("org.example", "parent-pom", Some("1.0.0"));
    let parent_xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>parent-pom</artifactId>
        <version>1.0.0</version>
        <properties>
            <slf4j.version>1.7.30</slf4j.version>
        </properties>
        <dependencyManagement>
            <dependencies>
                <dependency>
                    <groupId>org.slf4j</groupId>
                    <artifactId>slf4j-api</artifactId>
                    <version>${slf4j.version}</version>
                </dependency>
            </dependencies>
        </dependencyManagement>
    </project>"#;
    cache
        .store_file(&parent_coord, "parent-pom-1.0.0.pom", parent_xml.as_bytes())
        .unwrap();

    let child_xml = r#"<project>
        <parent>
            <groupId>org.example</groupId>
            <artifactId>parent-pom</artifactId>
            <version>1.0.0</version>
        </parent>
        <artifactId>child-service</artifactId>
        <properties>
            <slf4j.version>2.0.9</slf4j.version>
        </properties>
        <dependencies>
            <dependency>
                <groupId>org.slf4j</groupId>
                <artifactId>slf4j-api</artifactId>
            </dependency>
        </dependencies>
    </project>"#;

    let mut child_pom = PomModel::parse(child_xml).unwrap();
    child_pom.resolve_parent_from_cache(&cache);
    child_pom.interpolate(&ahash::AHashMap::default());

    let deps = child_pom.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "2.0.9");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
