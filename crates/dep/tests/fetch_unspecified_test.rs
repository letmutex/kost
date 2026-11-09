#[tokio::test]
async fn fetch_rejects_offline_flag() {
    let result = dep::run(&[
        "fetch".to_string(),
        "--offline".to_string(),
        "org.example:foo".to_string(),
    ])
    .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("--offline"));
}

#[tokio::test]
async fn fetch_resolves_unspecified_version_from_repository() {
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_test_fetch_unspecified_",
        &std::process::id().to_string()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let repo_dir = temp_dir.join("repo");
    let gradle_home = temp_dir.join("gradle_home");
    std::fs::create_dir_all(&gradle_home).unwrap();

    let meta_dir = repo_dir.join("org").join("example").join("demo");
    let v1_dir = meta_dir.join("1.0.0");
    let v2_dir = meta_dir.join("1.2.0");
    let v3_dir = meta_dir.join("1.3.0-alpha01");

    std::fs::create_dir_all(&v1_dir).unwrap();
    std::fs::create_dir_all(&v2_dir).unwrap();
    std::fs::create_dir_all(&v3_dir).unwrap();

    let metadata_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>org.example</groupId>
  <artifactId>demo</artifactId>
  <versioning>
    <latest>1.3.0-alpha01</latest>
    <release>1.2.0</release>
    <versions>
      <version>1.0.0</version>
      <version>1.2.0</version>
      <version>1.3.0-alpha01</version>
    </versions>
  </versioning>
</metadata>"#;
    std::fs::write(meta_dir.join("maven-metadata.xml"), metadata_xml).unwrap();

    let pom_xml = r#"<project>
  <groupId>org.example</groupId>
  <artifactId>demo</artifactId>
  <version>1.2.0</version>
</project>"#;
    std::fs::write(v2_dir.join("demo-1.2.0.pom"), pom_xml).unwrap();
    std::fs::write(v2_dir.join("demo-1.2.0.jar"), b"binary jar content").unwrap();

    let repo_path = repo_dir.display().to_string().replace('\\', "/");
    let repo_url = if repo_path.starts_with('/') {
        common::concat_str!("file://", &repo_path, "/")
    } else {
        common::concat_str!("file:///", &repo_path, "/")
    };

    let result = dep::run(&[
        "fetch".to_string(),
        "--gradle-home".to_string(),
        gradle_home.display().to_string(),
        "--maven-repo".to_string(),
        repo_url,
        "org.example:demo".to_string(),
    ])
    .await;

    assert!(result.is_ok(), "fetch should succeed: {:?}", result.err());

    let cache = dep::GradleCache::new(Some(&gradle_home)).unwrap();
    let solved_coord = dep::DependencyCoordinate::parse("org.example:demo:1.2.0").unwrap();
    assert!(
        cache.find_artifact(&solved_coord, false).is_some(),
        "Latest stable version 1.2.0 should be downloaded to cache"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn list_resolves_unspecified_version_from_cache_offline() {
    let temp_dir = std::env::temp_dir().join(common::concat_str!(
        "kost_test_unspecified_offline_",
        &std::process::id().to_string()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let gradle_home = temp_dir.join("gradle_home");
    let cache = dep::GradleCache::new(Some(&gradle_home)).unwrap();

    let cached_coord = dep::DependencyCoordinate::parse("org.example:local-lib:2.4.0").unwrap();
    cache
        .store_file(&cached_coord, "local-lib-2.4.0.pom", b"<project></project>")
        .unwrap();
    cache
        .store_file(&cached_coord, "local-lib-2.4.0.jar", b"cached content")
        .unwrap();

    let result = dep::run(&[
        "list".to_string(),
        "--gradle-home".to_string(),
        gradle_home.display().to_string(),
        "--maven-repo".to_string(),
        "https://repo.example.com/maven".to_string(),
        "--offline".to_string(),
        "--direct".to_string(),
        "org.example:local-lib".to_string(),
    ])
    .await;

    assert!(
        result.is_ok(),
        "list should resolve from cache when offline"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
