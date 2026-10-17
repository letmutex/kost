//! Raw deserialization schema for YAML configuration files.

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum RawInclude {
    /// String form: full ruleset/file ("standard", "./shared.yaml")
    /// or inline qualified rule import ("standard:no-semi", "./shared.yaml:disallow-todo").
    Simple(String),
    /// Structured form: target path + list of rule IDs.
    Structured { path: String, rules: Vec<String> },
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub lint: Option<LintSection>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintSection {
    #[serde(default)]
    pub includes: Vec<RawInclude>,
    #[serde(default)]
    pub rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRule {
    pub id: String,
    pub severity: Option<String>,
    pub allow_where_override: Option<bool>,
    pub options: Option<IndexMap<String, serde_yaml::Value>>,
    #[serde(rename = "where")]
    pub where_clause: Option<serde_yaml::Value>,
    pub message: Option<String>,
}
