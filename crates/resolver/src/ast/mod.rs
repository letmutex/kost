pub mod groovy;
pub mod groovy_meta;
pub mod groovy_prop;
pub mod groovy_repo;
pub mod kotlin;
pub mod kotlin_meta;
pub mod kotlin_prop;
pub mod kotlin_repo;
pub mod string;
pub mod util;

use ahash::AHashMap;
use std::fmt;
use std::path::Path;
use tree_sitter::Node;

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
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ParseError::UnknownDialect(path.to_string_lossy().to_string()))?;
        if name.ends_with(".kts") {
            Ok(ScriptDialect::Kotlin)
        } else if name.ends_with(".gradle") {
            Ok(ScriptDialect::Groovy)
        } else {
            Err(ParseError::UnknownDialect(name.to_string()))
        }
    }

    /// Detects script dialect from source content using Kotlin and Groovy AST parsers.
    pub fn detect_from_content(source: &str) -> Self {
        let source_bytes = source.as_bytes();
        let kt_tree =
            kotlin::KOTLIN_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));
        let gr_tree =
            groovy::GROOVY_PARSER.with(|parser| parser.borrow_mut().parse(source_bytes, None));

        match (kt_tree, gr_tree) {
            (Some(kt), Some(gr)) => {
                let kt_root = kt.root_node();
                let gr_root = gr.root_node();
                if !kt_root.has_error() && !has_groovy_only_tokens(gr_root, source_bytes) {
                    return Self::Kotlin;
                }
                if !util::has_unrecoverable_groovy_error(gr_root, source_bytes) {
                    return Self::Groovy;
                }
                if has_kotlin_specific_ast(kt_root) {
                    return Self::Kotlin;
                }
                if has_groovy_specific_ast(gr_root, source_bytes) {
                    return Self::Groovy;
                }
                Self::Kotlin
            }
            (None, Some(_)) => Self::Groovy,
            _ => Self::Kotlin,
        }
    }
}

fn has_groovy_only_tokens(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "map_literal" | "map_entry" | "labeled_statement" => return true,
        "local_variable_declaration" => {
            if let Some(first) = node.child(0)
                && util::node_text(first, source) == "def"
            {
                return true;
            }
        }
        "string_literal" if util::node_text(node, source).starts_with('\'') => {
            return true;
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_groovy_only_tokens(child, source) {
            return true;
        }
    }
    false
}

fn has_kotlin_specific_ast(node: Node) -> bool {
    if node.kind() == "property_declaration" {
        let mut prop_cursor = node.walk();
        if node
            .children(&mut prop_cursor)
            .any(|child| matches!(child.kind(), "val" | "var"))
        {
            return true;
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_kotlin_specific_ast(child) {
            return true;
        }
    }
    false
}

fn has_groovy_specific_ast(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "command_chain" => {
            if let Some(first) = node
                .child_by_field_name("receiver")
                .or_else(|| node.child(0))
                && first.kind() == "identifier"
            {
                let name = util::node_text(first, source).trim();
                if !matches!(
                    name,
                    "val" | "var" | "plugins" | "android" | "dependencies" | "repositories"
                ) {
                    return true;
                }
            }
        }
        "map_literal" | "map_entry" | "labeled_statement" => return true,
        "local_variable_declaration" => {
            if let Some(first) = node.child(0)
                && util::node_text(first, source) == "def"
            {
                return true;
            }
        }
        "string_literal" if util::node_text(node, source).starts_with('\'') => {
            return true;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_groovy_specific_ast(child, source) {
            return true;
        }
    }
    false
}

/// Extracted repository definitions from a build script.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepositoriesResult {
    pub urls: Vec<String>,
    pub google: bool,
    pub maven_central: bool,
    pub gradle_plugin_portal: bool,
    pub maven_local: bool,
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
        let dialect = ScriptDialect::detect_from_content(content);
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
