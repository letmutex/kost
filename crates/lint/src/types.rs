//! Core data types and contracts for the linting engine.

pub use crate::property::PropertyKey;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Severity level for rule diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    None, // Disabled rule; skipped during execution
}

impl Severity {
    pub fn parse(string: &str) -> Option<Self> {
        match string {
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::None => "none",
        }
    }

    pub fn color(self) -> Option<&'static str> {
        match self {
            Self::Error => Some(common::color::BOLD_RED),
            Self::Warning => Some(common::color::BOLD_YELLOW),
            Self::None => None,
        }
    }
}

/// 1-indexed source text position and span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl TextRange {
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    pub fn from_node(node: tree_sitter::Node) -> Self {
        let start = node.start_position();
        let end = node.end_position();
        Self {
            start_line: (start.row + 1) as u32,
            start_col: (start.column + 1) as u32,
            end_line: (end.row + 1) as u32,
            end_col: (end.column + 1) as u32,
        }
    }
}

/// A diagnostic emitted by a lint rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rule_id: SmolStr,
    pub severity: Severity,
    pub range: TextRange,
    pub message: String,
}

impl Diagnostic {
    /// Formats this diagnostic directly into a writer without intermediate heap allocations.
    pub fn write_formatted<W: std::fmt::Write>(
        &self,
        writer: &mut W,
        file_path: Option<&str>,
        colorize: bool,
    ) -> std::fmt::Result {
        if colorize {
            use common::color::{BOLD, DIM, RESET, YELLOW};
            if let Some(path) = file_path {
                write!(writer, "{path}{DIM}:{RESET}")?;
            }
            write!(
                writer,
                "{YELLOW}{}{RESET}{DIM}:{RESET}{YELLOW}{}{RESET}{DIM}:{RESET} ",
                self.range.start_line, self.range.start_col,
            )?;
            let sev_color = self.severity.color().unwrap_or(RESET);
            write!(
                writer,
                "{BOLD}[{}] {} ({RESET}{sev_color}{}{RESET}{BOLD}){RESET}",
                self.rule_id.as_str(),
                self.message.as_str(),
                self.severity.as_str(),
            )
        } else {
            if let Some(path) = file_path {
                write!(writer, "{path}:")?;
            }
            write!(
                writer,
                "{}:{}: [{}] {} ({})",
                self.range.start_line,
                self.range.start_col,
                self.rule_id.as_str(),
                self.message.as_str(),
                self.severity.as_str(),
            )
        }
    }

    /// Formats this diagnostic into a new String with an optional file path.
    pub fn format_with_path(&self, file_path: Option<&str>) -> String {
        let mut buffer = String::new();
        let _ = self.write_formatted(&mut buffer, file_path, false);
        buffer
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_formatted(formatter, None, false)
    }
}

/// Lexical scope holding measurable code properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    File,
    Class,
    Function,
    Property,
    Lambda,
    Import,
}

impl Scope {
    pub fn parse(string: &str) -> Option<Self> {
        match string {
            "file" => Some(Self::File),
            "class" => Some(Self::Class),
            "function" => Some(Self::Function),
            "property" | "variable" => Some(Self::Property),
            "lambda" => Some(Self::Lambda),
            "import" => Some(Self::Import),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Class => "class",
            Self::Function => "function",
            Self::Property => "property",
            Self::Lambda => "lambda",
            Self::Import => "import",
        }
    }
}

/// Dynamically typed property and option value.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Int(i64),
    Float(f64),
    Str(SmolStr),
    Bool(bool),
    StrList(SmallVec<[SmolStr; 4]>),
}

impl PropertyValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Int(_) => "Int",
            Self::Float(_) => "Float",
            Self::Str(_) => "Str",
            Self::Bool(_) => "Bool",
            Self::StrList(_) => "StrList",
        }
    }

    pub fn as_numeric(&self) -> Option<Numeric> {
        match self {
            Self::Int(value) => Some(Numeric::Int(*value)),
            Self::Float(value) => Some(Numeric::Float(*value)),
            _ => None,
        }
    }
}

/// Alias for option values, sharing identical representation with property values.
pub type OptionValue = PropertyValue;

/// Unified numeric representation supporting mixed integer/float comparisons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numeric {
    Int(i64),
    Float(f64),
}

impl Numeric {
    #[inline]
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Int(integer_value) => *integer_value as f64,
            Self::Float(float_value) => *float_value,
        }
    }
}

/// Dynamic or literal operand in condition evaluations.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand<T> {
    Literal(T),
    OptionRef(SmolStr),
}

/// Capability contract governing whether configuration files may override condition trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WherePolicy {
    AllowOverride,
    RejectOverride,
}

impl WherePolicy {
    pub fn from_allow_bool(allowed: bool) -> Self {
        if allowed {
            Self::AllowOverride
        } else {
            Self::RejectOverride
        }
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::AllowOverride)
    }
}

/// Classification of comments in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CommentType {
    Eol,
    Block,
    Kdoc,
}

impl CommentType {
    pub fn description(self) -> &'static str {
        match self {
            Self::Eol => "an EOL comment",
            Self::Block => "a block comment",
            Self::Kdoc => "a KDoc",
        }
    }
}
