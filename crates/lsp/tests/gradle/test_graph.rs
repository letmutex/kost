use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use lsp::gradle::dependency::DependencyCoordinate;
use lsp::gradle::descriptor::ModuleDescriptor;
use lsp::gradle::graph::DependencyGraph;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_temp_cache_root() -> PathBuf {
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "test_graph_cache_{}_{}_{}",
        std::process::id(),
        nanos,
        count
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_mock_descriptor(
    cache_root: &std::path::Path,
    group: &str,
    name: &str,
    version: &str,
    dependencies: &[(&str, &str, &str)],
) {
    let desc_dir = cache_root
        .join("caches")
        .join("modules-2")
        .join("metadata-2.107")
        .join("descriptors")
        .join(group)
        .join(name)
        .join(version)
        .join("hash_val");
    fs::create_dir_all(&desc_dir).unwrap();

    let bytes = ModuleDescriptor::create_test_descriptor_bytes(
        group,
        name,
        version,
        dependencies,
        &[&format!("{}-{}.jar", name, version)],
    );
    fs::write(desc_dir.join("descriptor.bin"), bytes).unwrap();
}

#[test]
fn test_graph_without_descriptors_returns_direct_dependencies() {
    let cache_root = create_temp_cache_root();
    let direct = vec![
        DependencyCoordinate::new("com.example", "lib-a", Some("1.0.0")),
        DependencyCoordinate::new("com.example", "lib-b", Some("2.0.0")),
    ];

    let result = DependencyGraph::resolve_transitive_graph(&direct, Some(&cache_root));
    assert_eq!(result.direct_count, 2);
    assert_eq!(result.transitive_count, 0);
    assert_eq!(result.resolved_dependencies.len(), 2);
    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_graph_multi_level_transitive_expansion() {
    let cache_root = create_temp_cache_root();

    write_mock_descriptor(
        &cache_root,
        "com.example",
        "app",
        "1.0.0",
        &[
            ("com.example", "core", "1.0.0"),
            ("org.utils", "tools", "2.0.0"),
        ],
    );

    write_mock_descriptor(
        &cache_root,
        "com.example",
        "core",
        "1.0.0",
        &[("org.logger", "log", "1.5.0")],
    );

    let direct = vec![DependencyCoordinate::new(
        "com.example",
        "app",
        Some("1.0.0"),
    )];
    let result = DependencyGraph::resolve_transitive_graph(&direct, Some(&cache_root));

    assert_eq!(result.direct_count, 1);
    assert_eq!(result.transitive_count, 3);
    assert_eq!(result.resolved_dependencies.len(), 4);
    assert_eq!(result.descriptors_parsed, 2);

    let names: Vec<String> = result
        .resolved_dependencies
        .iter()
        .map(|d| d.name.to_string())
        .collect();
    assert!(names.contains(&"app".to_string()));
    assert!(names.contains(&"core".to_string()));
    assert!(names.contains(&"tools".to_string()));
    assert!(names.contains(&"log".to_string()));

    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_graph_handles_circular_dependencies() {
    let cache_root = create_temp_cache_root();

    write_mock_descriptor(
        &cache_root,
        "com.cycle",
        "alpha",
        "1.0.0",
        &[("com.cycle", "beta", "1.0.0")],
    );
    write_mock_descriptor(
        &cache_root,
        "com.cycle",
        "beta",
        "1.0.0",
        &[("com.cycle", "alpha", "1.0.0")],
    );

    let direct = vec![DependencyCoordinate::new(
        "com.cycle",
        "alpha",
        Some("1.0.0"),
    )];
    let result = DependencyGraph::resolve_transitive_graph(&direct, Some(&cache_root));

    assert_eq!(result.resolved_dependencies.len(), 2);
    assert_eq!(result.transitive_count, 1);

    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_graph_version_conflict_resolves_to_newest() {
    let cache_root = create_temp_cache_root();

    write_mock_descriptor(
        &cache_root,
        "com.mod",
        "first",
        "1.0.0",
        &[("com.shared", "util", "1.2.0")],
    );
    write_mock_descriptor(
        &cache_root,
        "com.mod",
        "second",
        "1.0.0",
        &[("com.shared", "util", "1.5.0")],
    );

    let direct = vec![
        DependencyCoordinate::new("com.mod", "first", Some("1.0.0")),
        DependencyCoordinate::new("com.mod", "second", Some("1.0.0")),
    ];
    let result = DependencyGraph::resolve_transitive_graph(&direct, Some(&cache_root));

    assert_eq!(result.resolved_dependencies.len(), 3);
    let util_dep = result
        .resolved_dependencies
        .iter()
        .find(|d| d.name == "util")
        .unwrap();
    assert_eq!(util_dep.version.as_ref().map(|v| v.raw()), Some("1.5.0"));

    let _ = fs::remove_dir_all(&cache_root);
}

#[test]
fn test_graph_superseded_dependency_in_queue_not_re_expanded() {
    let cache_root = create_temp_cache_root();

    write_mock_descriptor(
        &cache_root,
        "com.root",
        "app",
        "1.0.0",
        &[("com.lib", "pkg", "1.0.0"), ("com.lib", "pkg", "2.0.0")],
    );
    write_mock_descriptor(
        &cache_root,
        "com.lib",
        "pkg",
        "1.0.0",
        &[("com.obsolete", "legacy", "0.9.0")],
    );
    write_mock_descriptor(&cache_root, "com.lib", "pkg", "2.0.0", &[]);

    let direct = vec![DependencyCoordinate::new("com.root", "app", Some("1.0.0"))];
    let result = DependencyGraph::resolve_transitive_graph(&direct, Some(&cache_root));

    let names: Vec<String> = result
        .resolved_dependencies
        .iter()
        .map(|d| d.name.to_string())
        .collect();

    assert!(names.contains(&"app".to_string()));
    assert!(names.contains(&"pkg".to_string()));
    assert!(
        !names.contains(&"legacy".to_string()),
        "Superseded version 1.0.0's dependencies should not leak into final graph"
    );

    let _ = fs::remove_dir_all(&cache_root);
}
