use common::concat_str;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub path: String,
    #[serde(default)]
    pub indexing: IndexingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexingConfig {
    #[serde(default = "default_true")]
    pub index_dependencies: bool,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            index_dependencies: true,
        }
    }
}

const fn default_true() -> bool {
    true
}

/// 0-indexed cursor position (line and character column) following LSP specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PositionConfig {
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeInferenceTest {
    pub name: String,
    pub file: String,
    pub variable: String,
    pub position: PositionConfig,
    pub expected_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeInferenceFixture {
    pub tests: Vec<TypeInferenceTest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HoverTest {
    pub name: String,
    pub file: String,
    pub symbol: String,
    pub position: PositionConfig,
    #[serde(default)]
    pub contains: Vec<String>,
    #[serde(default)]
    pub not_contains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HoverFixture {
    pub tests: Vec<HoverTest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedReferenceLocation {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReferencesTest {
    pub name: String,
    pub file: String,
    pub symbol: String,
    pub position: PositionConfig,
    #[serde(default)]
    pub include_declaration: bool,
    #[serde(default)]
    pub expected_subset: Vec<ExpectedReferenceLocation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReferencesFixture {
    pub tests: Vec<ReferencesTest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureKind {
    TypeInference,
    Hover,
    References,
}

impl FeatureKind {
    pub const fn directory_name(&self) -> &'static str {
        match self {
            Self::TypeInference => "type-inference",
            Self::Hover => "hover",
            Self::References => "references",
        }
    }

    pub fn from_directory_name(name: &str) -> Option<Self> {
        match name {
            "type-inference" => Some(Self::TypeInference),
            "hover" => Some(Self::Hover),
            "references" => Some(Self::References),
            _ => None,
        }
    }
}

pub fn strip_unc_prefix(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

pub fn load_project_config(config_file_path: &Path) -> Result<ProjectConfig, String> {
    let content = std::fs::read_to_string(config_file_path).map_err(|err| {
        concat_str!(
            "Failed to read project config at ",
            &config_file_path.to_string_lossy(),
            ": ",
            &err.to_string()
        )
    })?;
    toml::from_str(&content).map_err(|err| {
        concat_str!(
            "Failed to parse TOML in ",
            &config_file_path.to_string_lossy(),
            ": ",
            &err.to_string()
        )
    })
}

pub fn resolve_project_root_path(config_file_path: &Path, configured_path: &str) -> PathBuf {
    let configured = Path::new(configured_path);
    if configured.is_absolute() && configured.exists() {
        return std::fs::canonicalize(configured).unwrap_or_else(|_| configured.to_path_buf());
    }

    let config_dir = config_file_path.parent().unwrap_or_else(|| Path::new("."));
    let direct_relative = config_dir.join(configured);
    if direct_relative.exists() {
        return std::fs::canonicalize(&direct_relative).unwrap_or(direct_relative);
    }

    // Walk up from config_dir to search for the path relative to parent directories / repo root
    let mut current = Some(config_dir);
    while let Some(dir) = current {
        let candidate = dir.join(configured);
        if candidate.exists() {
            return std::fs::canonicalize(&candidate).unwrap_or(candidate);
        }
        current = dir.parent();
    }

    // Try from current working directory and its parents
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(configured);
        if candidate.exists() {
            return std::fs::canonicalize(&candidate).unwrap_or(candidate);
        }
        let mut cur = Some(cwd.as_path());
        while let Some(dir) = cur {
            let candidate = dir.join(configured);
            if candidate.exists() {
                return std::fs::canonicalize(&candidate).unwrap_or(candidate);
            }
            cur = dir.parent();
        }
    }

    direct_relative
}
