//! Presentation mode for diagnostics and diff reporting across multiple files.

use crate::error::CliError;

/// Presentation mode for output formatting and diagnostic reporting across multiple files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrintMode {
    /// Heuristic tiering based on number of changed or violated files (<=10: full, 11..=20: truncated, >20: collapsed).
    #[default]
    Auto,
    /// Always display full diffs or diagnostics for all files.
    Full,
    /// Always truncate diffs or diagnostics per file with a footer.
    Truncated,
    /// Always display only file path headers and an overall collapsed footer.
    Collapsed,
}

impl PrintMode {
    /// Parses a string identifier into a `PrintMode`.
    pub fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "full" => Some(Self::Full),
            "truncated" => Some(Self::Truncated),
            "collapsed" => Some(Self::Collapsed),
            _ => None,
        }
    }

    /// Parses a command line option value string into a `PrintMode`.
    pub fn parse_option(option_name: &str, raw_value: &str) -> Result<Self, CliError> {
        let trimmed = raw_value.trim();
        if trimmed.is_empty() {
            return Err(CliError::MissingOptionValue(option_name.to_string()));
        }
        Self::from_identifier(trimmed).ok_or_else(|| CliError::InvalidOptionValue {
            option: option_name.to_string(),
            value: raw_value.to_string(),
        })
    }
}
