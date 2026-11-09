use dep::{
    DependencyCoordinate, Downloader, GradleCache, RepositoryRegistry, SolvedGraph, TreeRenderer,
};

#[tokio::test]
async fn offline_graph_solution_and_tree_rendering() {
    let temp_dir = std::env::temp_dir().join("kost_test_offline_graph");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let registry = RepositoryRegistry::new();
    let downloader = Downloader::new(2);

    let dep_direct = DependencyCoordinate::parse("org.example:core:1.2.0").unwrap();
    let pom_xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>core</artifactId>
        <version>1.2.0</version>
        <dependencies>
            <dependency>
                <groupId>org.example</groupId>
                <artifactId>utils</artifactId>
                <version>2.0.0</version>
                <scope>compile</scope>
            </dependency>
        </dependencies>
    </project>"#;

    cache
        .store_file(&dep_direct, "core-1.2.0.pom", pom_xml.as_bytes())
        .expect("Failed storing pom");

    let dep_utils = DependencyCoordinate::parse("org.example:utils:2.0.0").unwrap();
    cache
        .store_file(&dep_utils, "utils-2.0.0.jar", b"dummy jar content")
        .expect("Failed storing jar");

    let graph = SolvedGraph::solve(&[dep_direct], &registry, &cache, &downloader, true).await;

    assert_eq!(graph.direct_count, 1);
    assert_eq!(graph.transitive_count, 1);
    assert_eq!(graph.nodes.len(), 2);

    let tree_output = TreeRenderer::render_tree(&graph, None, None, None);
    assert!(tree_output.contains("org.example:core:1.2.0"));
    assert!(tree_output.contains("org.example:utils:2.0.0"));
    assert!(tree_output.contains("└── "));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn direct_dependency_conflict_resolution() {
    let temp_dir = std::env::temp_dir().join("kost_test_direct_conflict");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let registry = RepositoryRegistry::new();
    let downloader = Downloader::new(2);

    let d_old = DependencyCoordinate::parse("org.example:foo:1.9.0").unwrap();
    let d_new = DependencyCoordinate::parse("org.example:foo:1.12.0").unwrap();

    let graph = SolvedGraph::solve(&[d_old, d_new], &registry, &cache, &downloader, true).await;
    assert_eq!(graph.direct_count, 1);
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(
        graph.nodes[0].coordinate.version.as_ref().unwrap().raw(),
        "1.12.0"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn path_traversal_rejection() {
    let temp_dir = std::env::temp_dir().join("kost_test_path_traversal");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");

    let evil_coord = DependencyCoordinate::new("../../root", "evil", Some("1.0.0"));
    let res = cache.store_file(&evil_coord, "evil-1.0.0.jar", b"malicious content");
    assert!(res.is_err());

    let safe_coord = DependencyCoordinate::new("org.example", "lib", Some("1.0.0"));
    let res_evil_file = cache.store_file(&safe_coord, "../evil.jar", b"malicious content");
    assert!(res_evil_file.is_err());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn kmp_candidate_probing_order() {
    let generic_coord =
        DependencyCoordinate::parse("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0").unwrap();
    let candidates = generic_coord.kmp_probe_candidates();
    assert_eq!(candidates[0], "kotlinx-coroutines-core");
    assert_eq!(candidates[1], "kotlinx-coroutines-core-jvm");
    assert_eq!(candidates[2], "kotlinx-coroutines-core-desktop");

    let explicit_jvm_coord =
        DependencyCoordinate::parse("org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.8.0")
            .unwrap();
    let jvm_candidates = explicit_jvm_coord.kmp_probe_candidates();
    assert_eq!(jvm_candidates, vec!["kotlinx-coroutines-core-jvm"]);
}

#[tokio::test]
async fn pom_dependency_management_version_inheritance() {
    let xml = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>parent-bom</artifactId>
        <version>1.0.0</version>
        <dependencyManagement>
            <dependencies>
                <dependency>
                    <groupId>org.example</groupId>
                    <artifactId>managed-lib</artifactId>
                    <version>3.5.0</version>
                </dependency>
            </dependencies>
        </dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.example</groupId>
                <artifactId>managed-lib</artifactId>
            </dependency>
        </dependencies>
    </project>"#;

    let mut model = dep::pom::PomModel::parse(xml).expect("POM parse failed");
    model.interpolate(&ahash::AHashMap::default());
    let deps = model.direct_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].name.as_str(), "managed-lib");
    assert_eq!(deps[0].version.as_ref().unwrap().raw(), "3.5.0");
}

#[tokio::test]
async fn tree_filter_matches_transitive_dependency() {
    let temp_dir = std::env::temp_dir().join("kost_test_filter_transitive");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");
    let registry = RepositoryRegistry::new();
    let downloader = Downloader::new(2);

    let dep_top = DependencyCoordinate::parse("com.example:parent:1.0.0").unwrap();
    let pom_xml = r#"<project>
        <groupId>com.example</groupId>
        <artifactId>parent</artifactId>
        <version>1.0.0</version>
        <dependencies>
            <dependency>
                <groupId>com.squareup.okio</groupId>
                <artifactId>okio</artifactId>
                <version>3.5.0</version>
            </dependency>
        </dependencies>
    </project>"#;

    cache
        .store_file(&dep_top, "parent-1.0.0.pom", pom_xml.as_bytes())
        .expect("Failed storing parent pom");

    let dep_okio = DependencyCoordinate::parse("com.squareup.okio:okio:3.5.0").unwrap();
    cache
        .store_file(&dep_okio, "okio-3.5.0.jar", b"okio content")
        .expect("Failed storing okio jar");

    let graph = SolvedGraph::solve(&[dep_top], &registry, &cache, &downloader, true).await;
    assert_eq!(graph.nodes.len(), 2);

    let filtered_tree = TreeRenderer::render_tree(&graph, None, None, Some("okio"));
    assert!(filtered_tree.contains("com.example:parent:1.0.0"));
    assert!(filtered_tree.contains("com.squareup.okio:okio:3.5.0"));

    let non_matching_tree = TreeRenderer::render_tree(&graph, None, None, Some("nonexistent"));
    assert!(non_matching_tree.trim().is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn cached_kmp_variant_lookup() {
    let temp_dir = std::env::temp_dir().join("kost_test_kmp_cache_lookup");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let cache = GradleCache::new(Some(&temp_dir)).expect("Failed creating test GradleCache");

    let jvm_coord =
        DependencyCoordinate::parse("org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.8.0")
            .unwrap();
    cache
        .store_file(
            &jvm_coord,
            "kotlinx-coroutines-core-jvm-1.8.0.jar",
            b"fake jvm jar",
        )
        .expect("Store jvm jar");

    let generic_coord =
        DependencyCoordinate::parse("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0").unwrap();

    let found = cache.find_artifact(&generic_coord, false);
    assert!(
        found.is_some(),
        "Generic coordinate should find cached -jvm variant"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn fetch_fails_fast_if_gradle_home_does_not_exist() {
    let non_existent_home = std::env::temp_dir().join("kost_test_nonexistent_gradle_home_xyz123");
    let _ = std::fs::remove_dir_all(&non_existent_home);

    let result = dep::run(&[
        "fetch".to_string(),
        "--gradle-home".to_string(),
        non_existent_home.display().to_string(),
        "org.example:foo:1.0.0".to_string(),
    ])
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Gradle home directory does not exist"));
    assert!(err.contains(&non_existent_home.display().to_string()));
}

#[tokio::test]
async fn dep_fails_when_no_repositories_configured() {
    let temp_dir = std::env::temp_dir().join("kost_test_no_repos_configured");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let script_file = temp_dir.join("build.gradle.kts");
    std::fs::write(
        &script_file,
        "dependencies { implementation(\"org.example:foo:1.0.0\") }\n",
    )
    .unwrap();

    let result = dep::run(&[
        "list".to_string(),
        "-p".to_string(),
        temp_dir.display().to_string(),
        "--gradle-home".to_string(),
        temp_dir.display().to_string(),
    ])
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("No repositories"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
