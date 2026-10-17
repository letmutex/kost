use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::gradle::build_script::{BuildScriptInfo, parse_gradle_properties};
use crate::gradle::catalog::VersionCatalog;
use crate::gradle::dependency::DependencyCoordinate;
use crate::gradle::version::ArtifactVersion;

pub fn discover_build_files(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut catalogs = Vec::new();
    let mut build_scripts = Vec::new();

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
                build_scripts.push(path.to_path_buf());
            }
        }
    }

    (catalogs, build_scripts)
}

pub fn collect_declared_dependencies(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
) -> (Vec<DependencyCoordinate>, Vec<String>, Option<u32>) {
    let mut primary_catalog = None;
    let mut all_deps = Vec::new();
    let mut kotlin_vers = Vec::new();
    let mut pref_sdk: Option<u32> = None;
    let mut shared_vars = ahash::AHashMap::default();

    for cat_path in catalogs {
        if let Ok(content) = std::fs::read_to_string(cat_path)
            && let Ok(catalog) = VersionCatalog::parse(&content)
        {
            all_deps.extend(catalog.all_dependencies());
            if primary_catalog.is_none() {
                primary_catalog = Some(catalog);
            }
        }
    }

    for script_path in scripts {
        let is_properties = script_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "gradle.properties");
        if is_properties && let Ok(content) = std::fs::read_to_string(script_path) {
            parse_gradle_properties(&content, &mut shared_vars);
        }
    }

    for script_path in scripts {
        let is_properties = script_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "gradle.properties");
        if is_properties {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(script_path) {
            let info =
                BuildScriptInfo::parse_with_vars(&content, primary_catalog.as_ref(), &shared_vars);
            shared_vars.extend(info.variables);
            all_deps.extend(info.dependencies);
            kotlin_vers.extend(info.kotlin_versions);
            if info.has_android_block
                && let Some(sdk) = info.compile_sdk
            {
                pref_sdk = Some(pref_sdk.map_or(sdk, |prev| prev.max(sdk)));
            }
        }
    }

    (all_deps, kotlin_vers, pref_sdk)
}

pub fn resolve_highest_kotlin_version(versions: &[String]) -> Option<String> {
    versions
        .iter()
        .max_by_key(|s| ArtifactVersion::parse(s))
        .cloned()
}

#[derive(Default)]
pub struct DiscoveryResult {
    pub catalogs: Vec<PathBuf>,
    pub scripts: Vec<PathBuf>,
    pub declared_dependencies: Vec<DependencyCoordinate>,
    pub kotlin_versions: Vec<String>,
    pub preferred_sdk: Option<u32>,
}

pub fn discover_workspace_pipeline_context(workspace_root: &Path) -> DiscoveryResult {
    let (catalogs, scripts) = discover_build_files(workspace_root);
    let (declared_dependencies, kotlin_versions, preferred_sdk) =
        collect_declared_dependencies(&catalogs, &scripts);
    DiscoveryResult {
        catalogs,
        scripts,
        declared_dependencies,
        kotlin_versions,
        preferred_sdk,
    }
}
