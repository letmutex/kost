pub mod groovy;
pub mod groovy_prop;
pub mod groovy_repo;
pub mod kotlin;
pub mod kotlin_repo;
pub mod util;

use std::fmt;
use std::path::Path;

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
