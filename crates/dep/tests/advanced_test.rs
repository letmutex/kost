use dep::{
    DependencyCoordinate, Downloader, GradleCache, RepositoryRegistry, SolvedGraph, TreeRenderer,
    pom::PomModel,
};

#[test]
fn test_pom_parent_coordinate_and_property_interpolation() {
    let xml = r#"<project>
        <parent>
            <groupId>org.example.parent</groupId>
            <artifactId>parent-module</artifactId>
            <version>2.5.0</version>
        </parent>
        <artifactId>child-module</artifactId>
        <dependencies>
            <dependency>
                <groupId>${project.groupId}</groupId>
                <artifactId>sibling-module</artifactId>
                <version>${project.version}</version>
            </dependency>
        </dependencies>
    </project>"#;

    let mut model = PomModel::parse(xml).expect("POM parse failed");
    model.interpolate(&ahash::AHashMap::default());

    assert_eq!(model.group.as_deref(), Some("org.example.parent"));
    assert_eq!(model.version.as_deref(), Some("2.5.0"));

    let deps = model.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].group.as_str(), "org.example.parent");
    assert_eq!(deps[0].name.as_str(), "sibling-module");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "2.5.0");
}

#[tokio::test]
async fn test_unversioned_direct_dependency_promoted_by_transitive() {
    let temp_dir = std::env::temp_dir().join("kost_test_unversioned_promotion");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let registry = RepositoryRegistry::new();
    let downloader = Downloader::new(2);

    let dep_direct = DependencyCoordinate::parse("org.example:feature:1.0.0").unwrap();
    let pom_direct = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>feature</artifactId>
        <version>1.0.0</version>
        <dependencies>
            <dependency>
                <groupId>org.example</groupId>
                <artifactId>core</artifactId>
                <version>2.1.0</version>
            </dependency>
        </dependencies>
    </project>"#;

    cache
        .store_file(&dep_direct, "feature-1.0.0.pom", pom_direct.as_bytes())
        .expect("Store pom");

    let dep_unversioned_core = DependencyCoordinate::new("org.example", "core", None);

    let graph = SolvedGraph::solve(
        &[dep_direct, dep_unversioned_core],
        &registry,
        &cache,
        &downloader,
        true,
    )
    .await;

    let core_node = graph
        .nodes
        .iter()
        .find(|n| n.coordinate.name.as_str() == "core")
        .expect("Core node present in solved graph");

    assert_eq!(
        core_node.coordinate.version.as_ref().map(|v| v.raw()),
        Some("2.1.0")
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_tree_filter_case_insensitive_and_version_match() {
    let temp_dir = std::env::temp_dir().join("kost_test_filter_case_version");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let registry = RepositoryRegistry::new();
    let downloader = Downloader::new(2);

    let dep_top = DependencyCoordinate::parse("org.example:MyService:3.4.1").unwrap();
    let graph = SolvedGraph::solve(&[dep_top], &registry, &cache, &downloader, true).await;

    let case_matched = TreeRenderer::render_tree(&graph, None, None, Some("myservice"));
    assert!(case_matched.contains("org.example:MyService:3.4.1"));

    let version_matched = TreeRenderer::render_tree(&graph, None, None, Some("3.4.1"));
    assert!(version_matched.contains("org.example:MyService:3.4.1"));

    let unmatched = TreeRenderer::render_tree(&graph, None, None, Some("4.0.0"));
    assert!(unmatched.trim().is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_path_sanitization_rejects_colon_and_dot() {
    let temp_dir = std::env::temp_dir().join("kost_test_path_sanitization");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Create cache");

    let colon_coord = DependencyCoordinate::new("C:", "foo", Some("1.0.0"));
    assert!(
        cache
            .store_file(&colon_coord, "foo-1.0.0.jar", b"test")
            .is_err()
    );

    let dot_coord = DependencyCoordinate::new(".", "foo", Some("1.0.0"));
    assert!(
        cache
            .store_file(&dot_coord, "foo-1.0.0.jar", b"test")
            .is_err()
    );

    let wildcard_coord = DependencyCoordinate::new("org.example", "foo", Some("1.0.*"));
    assert!(
        cache
            .store_file(&wildcard_coord, "foo-1.0.*.jar", b"test")
            .is_err()
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_pom_parent_property_variables_interpolation() {
    let xml = r#"<project>
        <parent>
            <groupId>org.example.bom</groupId>
            <artifactId>sample-bom</artifactId>
            <version>4.2.0</version>
        </parent>
        <artifactId>my-app</artifactId>
        <dependencies>
            <dependency>
                <groupId>${project.parent.groupId}</groupId>
                <artifactId>app-core</artifactId>
                <version>${project.parent.version}</version>
            </dependency>
            <dependency>
                <groupId>${parent.groupId}</groupId>
                <artifactId>app-util</artifactId>
                <version>${parent.version}</version>
            </dependency>
        </dependencies>
    </project>"#;

    let mut model = PomModel::parse(xml).expect("Parse pom");
    model.interpolate(&ahash::AHashMap::default());
    let deps = model.direct_dependencies();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].group.as_str(), "org.example.bom");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "4.2.0");
    assert_eq!(deps[1].group.as_str(), "org.example.bom");
    assert_eq!(deps[1].version.as_ref().unwrap().raw(), "4.2.0");
}

#[test]
fn test_script_dependencies_with_classifier() {
    let temp_dir = std::env::temp_dir().join("kost_test_script_classifier");
    let _ = std::fs::create_dir_all(&temp_dir);
    let script_file = temp_dir.join("build.gradle.kts");
    let script = r#"
        dependencies {
            implementation("io.netty:netty-transport-native-epoll:4.1.100.Final:linux-x86_64")
            api 'androidx.annotation:annotation-experimental:1.4.1@aar'
        }
    "#;
    std::fs::write(&script_file, script).unwrap();

    let deps = dep::discovery::collect_project_dependencies(&[], &[script_file]);
    assert_eq!(deps.len(), 2);
    let netty = deps
        .iter()
        .find(|d| d.name.as_str() == "netty-transport-native-epoll")
        .expect("Netty dep found");
    assert_eq!(netty.classifier.as_deref(), Some("linux-x86_64"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_repo_kotlin_dsl_shorthand_parsing() {
    let mut registry = RepositoryRegistry::new();
    let kts_content = r#"
        repositories {
            mavenCentral()
            maven("https://packages.jetbrains.team/maven/p/compose/dev")
            maven(url = "https://custom.mirror.org/repo")
        }
    "#;

    registry.parse_from_script(kts_content);

    let urls: Vec<&str> = registry
        .repositories
        .iter()
        .map(|r| r.url.as_str())
        .collect();
    assert!(urls.contains(&"https://repo1.maven.org/maven2/"));
    assert!(urls.contains(&"https://packages.jetbrains.team/maven/p/compose/dev/"));
    assert!(urls.contains(&"https://custom.mirror.org/repo/"));
}

#[test]
fn test_pom_cdata_section_parsing() {
    let xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>cdata-lib</artifactId>
        <version><![CDATA[1.5.0]]></version>
        <dependencies>
            <dependency>
                <groupId><![CDATA[org.example.sub]]></groupId>
                <artifactId>core</artifactId>
                <version><![CDATA[2.0.0]]></version>
            </dependency>
        </dependencies>
    </project>"#;

    let model = PomModel::parse(xml).expect("Parse cdata pom");
    assert_eq!(model.version.as_deref(), Some("1.5.0"));
    let deps = model.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].group.as_str(), "org.example.sub");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "2.0.0");
}

#[test]
fn test_tree_renderer_diamond_and_cycle_handling() {
    // Construct graph with a cycle: node 0 -> node 1 -> node 0
    let node0 = dep::GraphNode {
        coordinate: DependencyCoordinate::parse("org.example:a:1.0").unwrap(),
        is_direct: true,
        promoted_from: None,
        children: vec![1],
        cached_binary: None,
        cached_sources: None,
    };
    let node1 = dep::GraphNode {
        coordinate: DependencyCoordinate::parse("org.example:b:1.0").unwrap(),
        is_direct: false,
        promoted_from: None,
        children: vec![0],
        cached_binary: None,
        cached_sources: None,
    };
    let graph = SolvedGraph {
        nodes: vec![node0, node1],
        direct_count: 1,
        transitive_count: 1,
    };

    let rendered = TreeRenderer::render_tree(&graph, None, None, None);
    assert!(rendered.contains("org.example:a:1.0"));
    assert!(rendered.contains("org.example:b:1.0"));
    assert!(rendered.contains("(*)"));
}
