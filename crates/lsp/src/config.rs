use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexingLogLevel {
    None,
    #[default]
    Default,
    Verbose,
}

impl IndexingLogLevel {
    pub fn parse_exact(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "none" => Some(Self::None),
            "default" => Some(Self::Default),
            "verbose" => Some(Self::Verbose),
            _ => None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_verbose(&self) -> bool {
        matches!(self, Self::Verbose)
    }
}

impl FromStr for IndexingLogLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_exact(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InteractionLogLevel {
    #[default]
    None,
    Default,
    Verbose,
}

impl InteractionLogLevel {
    pub fn parse_exact(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "none" => Some(Self::None),
            "default" => Some(Self::Default),
            "verbose" => Some(Self::Verbose),
            _ => None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_verbose(&self) -> bool {
        matches!(self, Self::Verbose)
    }
}

impl FromStr for InteractionLogLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_exact(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableHoverTypeInference {
    #[default]
    Auto,
    Always,
    AlwaysFullName,
    AlwaysWithTrace,
}

impl VariableHoverTypeInference {
    pub fn parse_exact(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "always_full_name" => Some(Self::AlwaysFullName),
            "always_with_trace" => Some(Self::AlwaysWithTrace),
            _ => None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Auto)
    }

    pub fn is_full_name(&self) -> bool {
        matches!(self, Self::AlwaysFullName)
    }

    pub fn is_trace_enabled(&self) -> bool {
        matches!(self, Self::AlwaysWithTrace)
    }
}

impl FromStr for VariableHoverTypeInference {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_exact(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub indexing_logging: IndexingLogLevel,
    pub interaction_logging: InteractionLogLevel,
    pub variable_hover_type_inference: VariableHoverTypeInference,
    pub auto_import: bool,
    pub work_done_progress_supported: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            indexing_logging: IndexingLogLevel::default(),
            interaction_logging: InteractionLogLevel::default(),
            variable_hover_type_inference: VariableHoverTypeInference::default(),
            auto_import: true,
            work_done_progress_supported: false,
        }
    }
}

impl ServerConfig {
    pub fn from_init_options(init_opts: Option<&serde_json::Value>) -> Self {
        let mut config = Self::default();

        if let Some(opts) = init_opts {
            if let Some(level) =
                extract_option(opts, "indexing_logging", IndexingLogLevel::parse_exact)
            {
                config.indexing_logging = level;
            }
            if let Some(level) = extract_option(
                opts,
                "interaction_logging",
                InteractionLogLevel::parse_exact,
            ) {
                config.interaction_logging = level;
            }
            if let Some(opt) = extract_option(
                opts,
                "variable_hover_type_inference",
                VariableHoverTypeInference::parse_exact,
            ) {
                config.variable_hover_type_inference = opt;
            }
            if let Some(auto_import) =
                extract_bool_option(opts, &["auto_import", "completion_auto_import"])
            {
                config.auto_import = auto_import;
            }
        }

        config
    }
}

fn extract_option<T>(
    opts: &serde_json::Value,
    key: &str,
    parse: fn(&str) -> Option<T>,
) -> Option<T> {
    opts.get(key)?.as_str().and_then(parse)
}

fn extract_bool_option(opts: &serde_json::Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(val) = opts.get(*key) {
            if let Some(bool_val) = val.as_bool() {
                return Some(bool_val);
            }
            if let Some(str_val) = val.as_str() {
                match str_val.trim().to_ascii_lowercase().as_str() {
                    "true" => return Some(true),
                    "false" => return Some(false),
                    _ => {}
                }
            }
        }
    }
    None
}
