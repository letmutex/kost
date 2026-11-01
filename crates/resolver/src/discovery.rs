pub use crate::ast;
pub use crate::catalog;

use std::path::{Path, PathBuf};

use ahash::AHashMap;
use ignore::WalkBuilder;

pub use crate::ast::{BuildScriptInfo, ParseError, RepositoriesResult, ScriptDialect};
use crate::catalog::VersionCatalog;
use crate::coordinate::{DependencyCoordinate, deduplicate_newest};
use crate::version::ArtifactVersion;

pub mod script {
    use ahash::AHashMap;

    use crate::ast::{
        ParseError, RepositoriesResult, ScriptDialect,
        groovy::{parse_groovy_script, parse_groovy_script_with_repos},
        kotlin::{parse_kotlin_script, parse_kotlin_script_with_repos},
    };
    use crate::coordinate::DependencyCoordinate;

    /// Parse direct declared dependencies from a build script using AST traversal.
    pub fn parse_script_dependencies(
        content: &str,
        properties: &AHashMap<String, String>,
        deps: &mut Vec<DependencyCoordinate>,
        dialect: ScriptDialect,
    ) -> Result<(), ParseError> {
        match dialect {
            ScriptDialect::Kotlin => parse_kotlin_script(content, properties, deps),
            ScriptDialect::Groovy => parse_groovy_script(content, properties, deps),
        }
    }

    /// Parse dependencies and repositories in a single AST pass over a build script.
    pub fn parse_script_dependencies_and_repos(
        content: &str,
        properties: &AHashMap<String, String>,
        deps: Option<&mut Vec<DependencyCoordinate>>,
        repos: &mut RepositoriesResult,
        dialect: ScriptDialect,
    ) -> Result<(), ParseError> {
        match dialect {
            ScriptDialect::Kotlin => {
                parse_kotlin_script_with_repos(content, properties, deps, Some(repos))
            }
            ScriptDialect::Groovy => {
                parse_groovy_script_with_repos(content, properties, deps, Some(repos))
            }
        }
    }
}

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

    catalogs.sort_by(|a, b| (a.components().count(), a).cmp(&(b.components().count(), b)));
    scripts.sort_by(|a, b| (a.components().count(), a).cmp(&(b.components().count(), b)));
    (catalogs, scripts)
}

/// Alias for `discover_gradle_files`.
pub fn discover_build_files(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    discover_gradle_files(root)
}

/// Parse direct declared dependencies from build scripts and version catalogs.
pub fn collect_project_dependencies(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
) -> Result<Vec<DependencyCoordinate>, ParseError> {
    let (deps, _) = collect_project_dependencies_and_repos(catalogs, scripts, true)?;
    Ok(deps)
}

/// Collects project dependencies (if `collect_deps` is true) and repositories in a single pass.
pub fn collect_project_dependencies_and_repos(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
    collect_deps: bool,
) -> Result<(Vec<DependencyCoordinate>, RepositoriesResult), ParseError> {
    let mut deps = Vec::new();
    let mut repos = RepositoriesResult::default();
    let mut properties = collect_gradle_properties(scripts);
    let mut primary_catalog: Option<VersionCatalog> = None;

    if collect_deps {
        for cat_path in catalogs {
            if let Ok(content) = std::fs::read_to_string(cat_path)
                && let Ok(parsed_cat) = VersionCatalog::parse(&content)
            {
                deps.extend(parsed_cat.all_dependencies());
                if primary_catalog.is_none() {
                    primary_catalog = Some(parsed_cat);
                }
            }
        }
    }

    for script_path in scripts {
        if script_path.file_name().and_then(|name| name.to_str()) != Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            let dialect = ScriptDialect::from_path(script_path)?;
            let info = BuildScriptInfo::parse_with_dialect(
                &content,
                dialect,
                primary_catalog.as_ref(),
                &properties,
            )?;
            properties = info.variables;
            if collect_deps {
                deps.extend(info.dependencies);
            }
            ast::kotlin::merge_repositories(&mut repos, info.repositories);
        }
    }

    let final_deps = if collect_deps {
        deduplicate_newest(deps)
    } else {
        Vec::new()
    };
    Ok((final_deps, repos))
}

pub fn collect_gradle_properties(scripts: &[PathBuf]) -> AHashMap<String, String> {
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

pub fn parse_gradle_properties(content: &str, props: &mut AHashMap<String, String>) {
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

pub fn collect_declared_dependencies(
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
) -> (Vec<DependencyCoordinate>, Vec<String>, Option<u32>) {
    let mut primary_catalog = None;
    let mut all_deps = Vec::new();
    let mut kotlin_vers = Vec::new();
    let mut pref_sdk: Option<u32> = None;
    let mut shared_vars = collect_gradle_properties(scripts);

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
        if script_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "gradle.properties")
        {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(script_path) {
            let info = match ScriptDialect::from_path(script_path) {
                Ok(dialect) => BuildScriptInfo::parse_with_dialect_lenient(
                    &content,
                    dialect,
                    primary_catalog.as_ref(),
                    &shared_vars,
                ),
                Err(_) => BuildScriptInfo::parse_with_vars(
                    &content,
                    primary_catalog.as_ref(),
                    &shared_vars,
                ),
            };
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
        .max_by_key(|ver_str| ArtifactVersion::parse(ver_str))
        .cloned()
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveryResult {
    pub catalogs: Vec<PathBuf>,
    pub scripts: Vec<PathBuf>,
    pub declared_dependencies: Vec<DependencyCoordinate>,
    pub kotlin_versions: Vec<String>,
    pub preferred_sdk: Option<u32>,
}

pub fn discover_workspace_pipeline_context(workspace_root: &Path) -> DiscoveryResult {
    let (catalogs, scripts) = discover_gradle_files(workspace_root);
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
