pub mod catalog;
pub mod script;

use std::path::{Path, PathBuf};

use ahash::AHashMap;
use ignore::WalkBuilder;

use crate::coordinate::{DependencyCoordinate, deduplicate_newest};
use catalog::parse_catalog_dependencies;
use script::parse_script_dependencies;
pub(crate) use script::strip_script_comments;

/// Discovers Gradle build files, settings, and version catalogs across a project tree.
pub fn discover_gradle_files(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut catalogs = Vec::new();
    let mut scripts = Vec::new();

    let walker = WalkBuilder::new(root)
        .standard_filters(true)
        .parents(false)
        .threads(4)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            if name.ends_with(".versions.toml") {
                catalogs.push(path.to_path_buf());
            } else if matches!(
                name,
                "build.gradle"
                    | "build.gradle.kts"
                    | "settings.gradle"
                    | "settings.gradle.kts"
                    | "gradle.properties"
            ) {
                scripts.push(path.to_path_buf());
            }
        }
    }

    (catalogs, scripts)
}

/// Parse direct declared dependencies from build scripts and version catalogs.
pub fn collect_project_dependencies(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
) -> Vec<DependencyCoordinate> {
    let mut deps = Vec::new();
    let mut properties = AHashMap::default();

    for script_path in scripts {
        if script_path.file_name().and_then(|n| n.to_str()) == Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            parse_gradle_properties(&content, &mut properties);
        }
    }

    // 1. Collect from version catalogs (.versions.toml)
    for cat_path in catalogs {
        if let Ok(content) = std::fs::read_to_string(cat_path) {
            parse_catalog_dependencies(&content, &mut deps);
        }
    }

    // 2. Collect from build scripts
    for script_path in scripts {
        if script_path.file_name().and_then(|n| n.to_str()) != Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            parse_script_dependencies(&content, &properties, &mut deps);
        }
    }

    deduplicate_newest(deps)
}

fn parse_gradle_properties(content: &str, props: &mut AHashMap<String, String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            props.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
}
