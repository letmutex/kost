pub mod ast;
pub mod catalog;
pub mod script;

use std::path::{Path, PathBuf};

use ahash::AHashMap;
pub use ast::{ParseError, RepositoriesResult, ScriptDialect};
use ignore::WalkBuilder;

use crate::coordinate::{DependencyCoordinate, deduplicate_newest};
use catalog::parse_catalog_dependencies;
use script::{parse_script_dependencies, parse_script_dependencies_and_repos};

/// Discovers Gradle build files, settings, and version catalogs across a project tree.
pub fn discover_gradle_files(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut catalogs = Vec::new();
    let mut scripts = Vec::new();

    let walker = WalkBuilder::new(root)
        .standard_filters(true)
        .parents(false)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file()
            && let Some(file_name) = path.file_name().and_then(|name| name.to_str())
        {
            if file_name.ends_with(".versions.toml") {
                catalogs.push(path.to_path_buf());
            } else if matches!(
                file_name,
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

    catalogs.sort();
    scripts.sort();
    (catalogs, scripts)
}

/// Parse direct declared dependencies from build scripts and version catalogs.
pub fn collect_project_dependencies(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
) -> Result<Vec<DependencyCoordinate>, ParseError> {
    let mut deps = Vec::new();
    let properties = collect_gradle_properties(scripts);

    for cat_path in catalogs {
        if let Ok(content) = std::fs::read_to_string(cat_path) {
            parse_catalog_dependencies(&content, &mut deps);
        }
    }

    for script_path in scripts {
        if script_path.file_name().and_then(|name| name.to_str()) != Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            let dialect = ScriptDialect::from_path(script_path)?;
            parse_script_dependencies(&content, &properties, &mut deps, dialect)?;
        }
    }

    Ok(deduplicate_newest(deps))
}

/// Collects project dependencies (if `collect_deps` is true) and repositories in a single pass.
pub(crate) fn collect_project_dependencies_and_repos(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
    collect_deps: bool,
) -> Result<(Vec<DependencyCoordinate>, RepositoriesResult), ParseError> {
    let mut deps = Vec::new();
    let mut repos = RepositoriesResult::default();
    let properties = collect_gradle_properties(scripts);

    if collect_deps {
        for cat_path in catalogs {
            if let Ok(content) = std::fs::read_to_string(cat_path) {
                parse_catalog_dependencies(&content, &mut deps);
            }
        }
    }

    for script_path in scripts {
        if script_path.file_name().and_then(|name| name.to_str()) != Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            let dialect = ScriptDialect::from_path(script_path)?;
            let deps_target = if collect_deps { Some(&mut deps) } else { None };
            parse_script_dependencies_and_repos(
                &content,
                &properties,
                deps_target,
                &mut repos,
                dialect,
            )?;
        }
    }

    let final_deps = if collect_deps {
        deduplicate_newest(deps)
    } else {
        Vec::new()
    };
    Ok((final_deps, repos))
}

pub(crate) fn collect_gradle_properties(scripts: &[PathBuf]) -> AHashMap<String, String> {
    let mut properties = AHashMap::default();
    for script_path in scripts {
        if script_path.file_name().and_then(|name| name.to_str()) == Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            parse_gradle_properties(&content, &mut properties);
        }
    }
    properties
}

fn parse_gradle_properties(content: &str, props: &mut AHashMap<String, String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=').or_else(|| trimmed.split_once(':')) {
            props.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
}
