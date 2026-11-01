pub mod dialect;
pub mod groovy;
pub mod groovy_android;
pub mod groovy_dep;
pub mod groovy_meta;
pub mod groovy_plugins;
pub mod groovy_prop;
pub mod groovy_repo;
pub mod groovy_sub;
pub mod kotlin;
pub mod kotlin_meta;
pub mod kotlin_prop;
pub mod kotlin_repo;
pub mod string;
pub mod util;

use ahash::AHashMap;
use std::fmt;
use std::path::Path;

use crate::catalog::VersionCatalog;
use crate::coordinate::DependencyCoordinate;

/// Errors that can occur during script parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    SyntaxError(String),
    UnknownDialect(String),
    TreeSitterError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SyntaxError(msg) => write!(f, "Syntax error in script: {}", msg),
            Self::UnknownDialect(msg) => write!(f, "Unknown script dialect: {}", msg),
            Self::TreeSitterError(msg) => write!(f, "Tree-sitter parser failed: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

/// Supported build script dialects in Gradle projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptDialect {
    Kotlin,
    Groovy,
}

impl ScriptDialect {
    /// Detects script dialect from file path extension.
    pub fn from_path(path: &Path) -> Result<Self, ParseError> {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ParseError::UnknownDialect(path.to_string_lossy().to_string()))?;
        if file_name.ends_with(".kts") {
            Ok(ScriptDialect::Kotlin)
        } else if file_name.ends_with(".gradle") {
            Ok(ScriptDialect::Groovy)
        } else {
            Err(ParseError::UnknownDialect(file_name.to_string()))
        }
    }

    /// Detects script dialect from source content using Kotlin and Groovy AST parsers.
    pub fn detect_from_content(source: &str) -> Self {
        dialect::detect_dialect_with_trees(source).0
    }
}

/// A single repository declaration extracted in script order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclaredRepository {
    Google,
    MavenCentral,
    GradlePluginPortal,
    MavenLocal,
    JCenter,
    MavenUrl(String),
}

/// Extracted repository definitions from a build script.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepositoriesResult {
    pub entries: Vec<DeclaredRepository>,
    pub plugin_entries: Vec<DeclaredRepository>,
    pub urls: Vec<String>,
    pub google: bool,
    pub maven_central: bool,
    pub gradle_plugin_portal: bool,
    pub maven_local: bool,
    pub jcenter: bool,
}

impl RepositoriesResult {
    pub fn record(&mut self, entry: DeclaredRepository, is_plugin_scope: bool) {
        match &entry {
            DeclaredRepository::Google => self.google = true,
            DeclaredRepository::MavenCentral => self.maven_central = true,
            DeclaredRepository::GradlePluginPortal => self.gradle_plugin_portal = true,
            DeclaredRepository::MavenLocal => self.maven_local = true,
            DeclaredRepository::JCenter => self.jcenter = true,
            DeclaredRepository::MavenUrl(url) => self.urls.push(url.clone()),
        }
        if is_plugin_scope {
            self.plugin_entries.push(entry);
        } else {
            self.entries.push(entry);
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.entries.extend(other.entries);
        self.plugin_entries.extend(other.plugin_entries);
        self.urls.extend(other.urls);
        self.google |= other.google;
        self.maven_central |= other.maven_central;
        self.gradle_plugin_portal |= other.gradle_plugin_portal;
        self.maven_local |= other.maven_local;
        self.jcenter |= other.jcenter;
    }

    pub fn ordered_entries(&self) -> impl Iterator<Item = &DeclaredRepository> {
        self.entries.iter().chain(self.plugin_entries.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self.plugin_entries.is_empty()
            && self.urls.is_empty()
            && !self.google
            && !self.maven_central
            && !self.gradle_plugin_portal
            && !self.maven_local
            && !self.jcenter
    }
}

/// Information extracted from a `.gradle` or `.gradle.kts` build script.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildScriptInfo {
    pub dependencies: Vec<DependencyCoordinate>,
    pub catalog_accessors: Vec<String>,
    pub kotlin_versions: Vec<String>,
    pub compile_sdk: Option<u32>,
    pub has_android_block: bool,
    pub plugins: Vec<(String, Option<String>)>,
    pub variables: AHashMap<String, String>,
    pub repositories: RepositoriesResult,
}

impl BuildScriptInfo {
    /// Parse a Gradle script content (`.gradle` or `.gradle.kts`).
    pub fn parse(content: &str, catalog: Option<&VersionCatalog>) -> Self {
        Self::parse_with_vars(content, catalog, &AHashMap::default())
    }

    /// Parse a Gradle script with inherited variables from parent project or `gradle.properties`.
    pub fn parse_with_vars(
        content: &str,
        catalog: Option<&VersionCatalog>,
        initial_vars: &AHashMap<String, String>,
    ) -> Self {
        let (dialect, kotlin_tree, groovy_tree) = dialect::detect_dialect_with_trees(content);
        let source_bytes = content.as_bytes();
        match dialect {
            ScriptDialect::Kotlin => {
                if let Some(tree) = kotlin_tree {
                    return kotlin::parse_kotlin_build_script_root(
                        tree.root_node(),
                        source_bytes,
                        catalog,
                        initial_vars,
                    );
                }
            }
            ScriptDialect::Groovy => {
                if let Some(tree) = groovy_tree {
                    return groovy::parse_groovy_build_script_root(
                        tree.root_node(),
                        source_bytes,
                        catalog,
                        initial_vars,
                    );
                }
            }
        }
        Self::parse_with_dialect_lenient(content, dialect, catalog, initial_vars)
    }

    /// Parse a Gradle script with an explicit dialect, returning `ParseError` on syntax errors.
    pub fn parse_with_dialect(
        content: &str,
        dialect: ScriptDialect,
        catalog: Option<&VersionCatalog>,
        initial_vars: &AHashMap<String, String>,
    ) -> Result<Self, ParseError> {
        match dialect {
            ScriptDialect::Kotlin => {
                kotlin::parse_kotlin_build_script(content, catalog, initial_vars, true)
            }
            ScriptDialect::Groovy => {
                groovy::parse_groovy_build_script(content, catalog, initial_vars, true)
            }
        }
    }

    /// Parse a Gradle script with an explicit dialect without failing on partial syntax errors.
    pub fn parse_with_dialect_lenient(
        content: &str,
        dialect: ScriptDialect,
        catalog: Option<&VersionCatalog>,
        initial_vars: &AHashMap<String, String>,
    ) -> Self {
        let res = match dialect {
            ScriptDialect::Kotlin => {
                kotlin::parse_kotlin_build_script(content, catalog, initial_vars, false)
            }
            ScriptDialect::Groovy => {
                groovy::parse_groovy_build_script(content, catalog, initial_vars, false)
            }
        };
        res.unwrap_or_default()
    }
}
