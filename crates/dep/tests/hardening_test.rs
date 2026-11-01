use dep::pom::PomModel;
use dep::{DependencyCoordinate, Downloader, GradleCache};

#[test]
fn test_zero_byte_cache_detection_and_corrupt_file_replacement() {
    let temp_dir = std::env::temp_dir().join("kost_test_zero_byte_cache");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let coord = DependencyCoordinate::parse("com.example:zero-byte-lib:1.0.0").unwrap();

    let target_dir = cache
        .files_dir()
        .join("com.example")
        .join("zero-byte-lib")
        .join("1.0.0")
        .join("0123456789abcdef0123456789abcdef01234567");
    std::fs::create_dir_all(&target_dir).expect("create target dir");
    let corrupt_file = target_dir.join("zero-byte-lib-1.0.0.jar");
    std::fs::write(&corrupt_file, b"").expect("write 0-byte file");

    assert!(cache.find_artifact(&coord, false).is_none());

    let real_content = b"valid jar content";
    let stored_path = cache
        .store_file(&coord, "zero-byte-lib-1.0.0.jar", real_content)
        .expect("store_file should succeed and replace corrupt file");

    assert!(stored_path.is_file());
    let read_back = std::fs::read(&stored_path).expect("read back");
    assert_eq!(read_back, real_content);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_aar_fallback_resolution_in_cache() {
    let temp_dir = std::env::temp_dir().join("kost_test_aar_fallback");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let coord = DependencyCoordinate::parse("com.google.android.material:material:1.11.0").unwrap();

    let aar_data = b"PK android archive content";
    let stored = cache
        .store_file(&coord, "material-1.11.0.aar", aar_data)
        .expect("Store aar");
    assert!(stored.is_file());

    let found = cache.find_artifact(&coord, false);
    assert!(found.is_some());
    assert_eq!(found.unwrap(), stored);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_downloader_utf8_lossy_fallback() {
    let temp_file = std::env::temp_dir().join("kost_test_legacy_iso88591.pom");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"<project><description>Cr\xe9dit Agricole</description></project>");
    std::fs::write(&temp_file, &bytes).expect("write ISO-8859-1 file");

    let downloader = Downloader::new(1);
    let mut url = common::concat_str!("file:///", &temp_file.to_string_lossy().replace('\\', "/"));
    if !url.starts_with("file:///") {
        url = common::concat_str!("file://", &temp_file.to_string_lossy());
    }

    let text = downloader.fetch_text(&url).await.expect("Fetch text");
    assert!(text.contains("Cr\u{fffd}dit Agricole"));

    let pom = PomModel::parse(&text).expect("Parse pom with replacement char");
    assert_eq!(pom.packaging.as_str(), "jar");
    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_pom_unprefixed_version_and_group_interpolation() {
    let xml = r#"<project>
        <groupId>com.legacy.example</groupId>
        <artifactId>parent-pkg</artifactId>
        <version>3.2.1</version>
        <dependencies>
            <dependency>
                <groupId>${groupId}</groupId>
                <artifactId>sub-module</artifactId>
                <version>${version}</version>
            </dependency>
        </dependencies>
    </project>"#;

    let mut pom = PomModel::parse(xml).expect("parse pom");
    pom.interpolate(&ahash::AHashMap::default());
    let deps = pom.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].group.as_str(), "com.legacy.example");
    assert_eq!(deps[0].name.as_str(), "sub-module");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "3.2.1");
}

#[tokio::test]
async fn test_kmp_variant_coordination_in_solved_graph() {
    let temp_dir = std::env::temp_dir().join("kost_test_kmp_graph_coord");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let cache = GradleCache::new(Some(&temp_dir)).expect("cache");
    let registry = dep::RepositoryRegistry::new();
    let downloader = Downloader::new(2);

    let dep_direct = DependencyCoordinate::parse("org.example:core:1.5.0").unwrap();
    let pom_direct = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>core</artifactId>
        <version>1.5.0</version>
        <dependencies>
            <dependency>
                <groupId>org.example</groupId>
                <artifactId>core-jvm</artifactId>
                <version>1.5.0</version>
            </dependency>
        </dependencies>
    </project>"#;

    cache
        .store_file(&dep_direct, "core-1.5.0.pom", pom_direct.as_bytes())
        .expect("store direct pom");

    let graph = dep::SolvedGraph::solve(&[dep_direct], &registry, &cache, &downloader, true).await;
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].coordinate.name.as_str(), "core-jvm");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_pom_xml_parsing_and_interpolation() {
    let xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>sample-lib</artifactId>
        <version>1.0.0</version>
        <properties>
            <coroutines.version>1.8.0</coroutines.version>
        </properties>
        <dependencies>
            <dependency>
                <groupId>org.jetbrains.kotlinx</groupId>
                <artifactId>kotlinx-coroutines-core</artifactId>
                <version>${coroutines.version}</version>
                <scope>compile</scope>
            </dependency>
            <dependency>
                <groupId>junit</groupId>
                <artifactId>junit</artifactId>
                <version>4.13.2</version>
                <scope>test</scope>
            </dependency>
        </dependencies>
    </project>"#;

    let mut pom = PomModel::parse(xml).unwrap();
    pom.interpolate(&ahash::AHashMap::default());

    assert_eq!(pom.group.as_deref(), Some("org.example"));
    assert_eq!(pom.name.as_deref(), Some("sample-lib"));
    let deps = pom.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name, "kotlinx-coroutines-core");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "1.8.0");
}

#[test]
fn test_pom_parent_dependency_management_inheritance() {
    let parent_xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>parent-bom</artifactId>
        <version>1.0.0</version>
        <dependencyManagement>
            <dependencies>
                <dependency>
                    <groupId>org.example</groupId>
                    <artifactId>unversioned-child</artifactId>
                    <version>2.5.0</version>
                </dependency>
            </dependencies>
        </dependencyManagement>
    </project>"#;

    let child_xml = r#"<project>
        <parent>
            <groupId>org.example</groupId>
            <artifactId>parent-bom</artifactId>
            <version>1.0.0</version>
        </parent>
        <artifactId>child-service</artifactId>
        <dependencies>
            <dependency>
                <groupId>org.example</groupId>
                <artifactId>unversioned-child</artifactId>
            </dependency>
        </dependencies>
    </project>"#;

    let parent_model = PomModel::parse(parent_xml).expect("parse parent");
    let mut child_model = PomModel::parse(child_xml).expect("parse child");
    child_model.inherit_from_parent(&parent_model);
    child_model.interpolate(&ahash::AHashMap::default());

    let deps = child_model.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name.as_str(), "unversioned-child");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "2.5.0");
}

#[test]
fn test_pom_ignores_plugin_and_profile_dependencies() {
    let xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>plugin-heavy-pom</artifactId>
        <version>1.0.0</version>
        <dependencyManagement>
            <dependencies>
                <dependency>
                    <groupId>org.example</groupId>
                    <artifactId>real-dep</artifactId>
                    <version>1.2.3</version>
                </dependency>
            </dependencies>
        </dependencyManagement>
        <build>
            <plugins>
                <plugin>
                    <groupId>org.apache.maven.plugins</groupId>
                    <artifactId>maven-compiler-plugin</artifactId>
                    <dependencies>
                        <dependency>
                            <groupId>org.codehaus.plexus</groupId>
                            <artifactId>plexus-compiler</artifactId>
                            <version>9.9.9</version>
                            <scope>test</scope>
                            <optional>true</optional>
                        </dependency>
                    </dependencies>
                </plugin>
            </plugins>
        </build>
        <dependencies>
            <dependency>
                <groupId>org.example</groupId>
                <artifactId>real-dep</artifactId>
            </dependency>
        </dependencies>
    </project>"#;

    let mut pom = PomModel::parse(xml).expect("parse pom");
    pom.interpolate(&ahash::AHashMap::default());
    let deps = pom.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].group.as_str(), "org.example");
    assert_eq!(deps[0].name.as_str(), "real-dep");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "1.2.3");
}

#[test]
fn test_module_metadata_available_at_redirect_resolution() {
    let json = r#"{
        "formatVersion": "1.1",
        "component": {
            "group": "org.jetbrains.kotlinx",
            "module": "kotlinx-coroutines-core",
            "version": "1.8.0"
        },
        "variants": [
            {
                "name": "jvmRuntimeElements-published",
                "available-at": {
                    "url": "../../kotlinx-coroutines-core-jvm/1.8.0/kotlinx-coroutines-core-jvm-1.8.0.module",
                    "group": "org.jetbrains.kotlinx",
                    "module": "kotlinx-coroutines-core-jvm",
                    "version": "1.8.0"
                }
            }
        ]
    }"#;

    let meta = dep::module::GradleModuleMetadata::parse(json).expect("parse module");
    let deps = meta.resolved_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].group.as_str(), "org.jetbrains.kotlinx");
    assert_eq!(deps[0].name.as_str(), "kotlinx-coroutines-core-jvm");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "1.8.0");
}

#[test]
fn test_repo_script_parsing_ignores_commented_repositories() {
    let mut registry = dep::RepositoryRegistry::new();
    let script = r#"
        repositories {
            // maven("https://commented.out.example.com/repo")
            /* maven("https://block.commented.example.com/repo") */
            maven("https://active.example.com/repo")
        }
    "#;
    registry
        .parse_from_script(script, dep::ScriptDialect::Kotlin)
        .unwrap();
    assert_eq!(registry.repositories.len(), 1);
    assert_eq!(
        registry.repositories[0].url,
        "https://active.example.com/repo/"
    );
}
