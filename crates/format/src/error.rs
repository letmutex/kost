/// 1-indexed position for user-facing diagnostic reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// 1-based line index
    pub line: usize,
    /// 1-based column index
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum FormatError {
    #[error("Parse error at line {}, column {}: {}", position.line, position.column, message)]
    ParseError { position: Position, message: String },
    #[error("Internal formatting error: {0}")]
    InternalError(String),
}
