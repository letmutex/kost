use std::fs;
use std::time::SystemTime;

use lsp::gradle::dependency::DependencyCoordinate;
use lsp::gradle::descriptor::ModuleDescriptor;

#[test]
fn test_parse_mock_descriptor_bytes() {
    let bytes = ModuleDescriptor::create_test_descriptor_bytes(
        "androidx.core",
        "core-ktx",
        "1.15.0",
        &[
            ("androidx.annotation", "annotation", "1.3.0"),
            ("androidx.core", "core", "1.15.0"),
            ("org.jetbrains.kotlin", "kotlin-stdlib", "2.0.20"),
        ],
        &["core-ktx-1.15.0.aar", "core-ktx-1.15.0-sources.jar"],
    );

    let desc = ModuleDescriptor::parse_bytes(&bytes).expect("Should parse valid descriptor bytes");
    assert_eq!(desc.coordinate.group, "androidx.core");
    assert_eq!(desc.coordinate.name, "core-ktx");
    assert_eq!(
        desc.coordinate.version.as_ref().map(|v| v.raw()),
        Some("1.15.0")
    );

    assert_eq!(desc.dependencies.len(), 3);
    assert!(
        desc.dependencies
            .iter()
            .any(|d| d.group == "androidx.annotation"
                && d.name == "annotation"
                && d.version.as_ref().map(|v| v.raw()) == Some("1.3.0"))
    );
    assert!(desc.dependencies.iter().any(|d| d.group == "androidx.core"
        && d.name == "core"
        && d.version.as_ref().map(|v| v.raw()) == Some("1.15.0")));
    assert!(
        desc.dependencies
            .iter()
            .any(|d| d.group == "org.jetbrains.kotlin"
                && d.name == "kotlin-stdlib"
                && d.version.as_ref().map(|v| v.raw()) == Some("2.0.20"))
    );

    assert!(desc.artifacts.contains(&"core-ktx-1.15.0.aar".to_string()));
    assert!(
        desc.artifacts
            .contains(&"core-ktx-1.15.0-sources.jar".to_string())
    );
}

#[test]
fn test_parse_corrupted_or_truncated_descriptor_bytes() {
    assert!(ModuleDescriptor::parse_bytes(&[]).is_none());
    assert!(ModuleDescriptor::parse_bytes(&[0x02]).is_none());
    assert!(ModuleDescriptor::parse_bytes(&[0x02, 0x85, 0x61, 0x62]).is_none());
    assert!(ModuleDescriptor::parse_bytes(&[0xFF, 0xFF, 0xFF, 0xFF]).is_none());
}

#[test]
fn test_find_descriptor_in_cache_hierarchy() {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cache_root = std::env::temp_dir().join(format!("test_cache_{}", nanos));
    fs::create_dir_all(&cache_root).unwrap();

    let desc_dir = cache_root
        .join("caches")
        .join("modules-2")
        .join("metadata-2.107")
        .join("descriptors")
        .join("com.google.code.gson")
        .join("gson")
        .join("2.10.1")
        .join("aabbccddeeff");
    fs::create_dir_all(&desc_dir).unwrap();

    let bytes = ModuleDescriptor::create_test_descriptor_bytes(
        "com.google.code.gson",
        "gson",
        "2.10.1",
        &[("junit", "junit", "4.13.2")],
        &["gson-2.10.1.jar"],
    );
    fs::write(desc_dir.join("descriptor.bin"), bytes).unwrap();

    let coord = DependencyCoordinate::new("com.google.code.gson", "gson", Some("2.10.1"));
    let found = ModuleDescriptor::find_in_cache(&cache_root, &coord);
    assert!(
        found.is_some(),
        "Descriptor should be located in metadata-2.107 cache"
    );

    let desc = found.unwrap();
    assert_eq!(desc.coordinate.group, "com.google.code.gson");
    assert_eq!(desc.coordinate.name, "gson");
    assert_eq!(desc.dependencies.len(), 1);
    assert_eq!(desc.dependencies[0].group, "junit");

    let missing_coord = DependencyCoordinate::new("com.missing", "lib", Some("1.0.0"));
    assert!(ModuleDescriptor::find_in_cache(&cache_root, &missing_coord).is_none());

    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_metadata_dirs_sorted_newest_numerical_version() {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cache_root = std::env::temp_dir().join(format!("test_meta_sort_{}", nanos));
    let modules_dir = cache_root.join("caches").join("modules-2");
    fs::create_dir_all(modules_dir.join("metadata-2.97")).unwrap();
    fs::create_dir_all(modules_dir.join("metadata-2.107")).unwrap();
    fs::create_dir_all(modules_dir.join("metadata-2.16")).unwrap();

    let discovered = ModuleDescriptor::discover_metadata_dirs(&cache_root);
    assert_eq!(discovered.len(), 3);
    assert_eq!(
        discovered[0].file_name().and_then(|n| n.to_str()),
        Some("metadata-2.107")
    );
    assert_eq!(
        discovered[1].file_name().and_then(|n| n.to_str()),
        Some("metadata-2.97")
    );
    assert_eq!(
        discovered[2].file_name().and_then(|n| n.to_str()),
        Some("metadata-2.16")
    );

    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_find_descriptor_without_version_selects_highest_semver() {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cache_root = std::env::temp_dir().join(format!("test_semver_desc_{}", nanos));
    let desc_base = cache_root
        .join("caches")
        .join("modules-2")
        .join("metadata-2.107")
        .join("descriptors")
        .join("com.example")
        .join("lib");

    let dir_1_9 = desc_base.join("1.9.0").join("hash1");
    let dir_1_10 = desc_base.join("1.10.0").join("hash2");
    fs::create_dir_all(&dir_1_9).unwrap();
    fs::create_dir_all(&dir_1_10).unwrap();

    let bytes_1_9 = ModuleDescriptor::create_test_descriptor_bytes(
        "com.example",
        "lib",
        "1.9.0",
        &[],
        &["lib-1.9.0.jar"],
    );
    let bytes_1_10 = ModuleDescriptor::create_test_descriptor_bytes(
        "com.example",
        "lib",
        "1.10.0",
        &[],
        &["lib-1.10.0.jar"],
    );
    fs::write(dir_1_9.join("descriptor.bin"), bytes_1_9).unwrap();
    fs::write(dir_1_10.join("descriptor.bin"), bytes_1_10).unwrap();

    let coord_no_version = DependencyCoordinate::new("com.example", "lib", None);
    let found = ModuleDescriptor::find_in_cache(&cache_root, &coord_no_version)
        .expect("Should resolve descriptor without version");
    assert_eq!(
        found.coordinate.version.as_ref().map(|v| v.raw()),
        Some("1.10.0"),
        "Should select 1.10.0 over 1.9.0 by semantic versioning"
    );

    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_parse_descriptor_with_bracket_version_range() {
    let bytes = ModuleDescriptor::create_test_descriptor_bytes(
        "com.example",
        "root",
        "1.0.0",
        &[("org.dep", "exact", "[1.5.0]")],
        &[],
    );
    let desc = ModuleDescriptor::parse_bytes(&bytes)
        .expect("Should parse descriptor with bracketed version");
    assert_eq!(desc.dependencies.len(), 1);
    assert_eq!(desc.dependencies[0].name, "exact");
}
