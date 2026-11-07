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
    if configured.is_absolute() {
        let canonical =
            std::fs::canonicalize(configured).unwrap_or_else(|_| configured.to_path_buf());
        return strip_unc_prefix(&canonical);
    }

    let config_dir = config_file_path.parent().unwrap_or_else(|| Path::new("."));
    let direct_relative = config_dir.join(configured);
    let canonical = std::fs::canonicalize(&direct_relative).unwrap_or(direct_relative);
    strip_unc_prefix(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_project_root_path_anchored_relative() {
        let temp_dir = std::env::temp_dir().join("test_lsp_config_rel");
        let project_dir = temp_dir.join("fixtures").join("my_project");
        let target_dir = temp_dir.join("repos").join("my_target");
        let _ = std::fs::create_dir_all(&project_dir);
        let _ = std::fs::create_dir_all(&target_dir);

        let config_file = project_dir.join("project.toml");
        let resolved = resolve_project_root_path(&config_file, "../../repos/my_target");
        let expected = strip_unc_prefix(&std::fs::canonicalize(&target_dir).unwrap());
        assert_eq!(resolved, expected);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_project_root_path_absolute() {
        let temp_dir = std::env::temp_dir().join("test_lsp_config_abs");
        let _ = std::fs::create_dir_all(&temp_dir);

        let dummy_config = temp_dir.join("dummy").join("project.toml");
        let resolved = resolve_project_root_path(&dummy_config, temp_dir.to_str().unwrap());
        let expected = strip_unc_prefix(&std::fs::canonicalize(&temp_dir).unwrap());
        assert_eq!(resolved, expected);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
