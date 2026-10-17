//! Presentation mode for diagnostics and diff reporting across multiple files.

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
}
