//! Diagnostic types, target anchors, and formatting for lint violations.

use crate::types::{Scope, Severity, TextRange};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Concrete AST element or location targeted for diagnostic reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportTarget {
    // File
    FileMaxIndentSpacesAt,
    FileStart,
    FileEnd,
    // Function
    FunctionName,
    FunctionStart,
    FunctionEnd,
    // Class
    ClassName,
    ClassStart,
    ClassEnd,
    // Variable
    VariableName,
    VariableStart,
    VariableEnd,
}

impl ReportTarget {
    pub fn scope(&self) -> Scope {
        match self {
            Self::FileMaxIndentSpacesAt | Self::FileStart | Self::FileEnd => Scope::File,
            Self::FunctionName | Self::FunctionStart | Self::FunctionEnd => Scope::Function,
            Self::ClassName | Self::ClassStart | Self::ClassEnd => Scope::Class,
            Self::VariableName | Self::VariableStart | Self::VariableEnd => Scope::Variable,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "file.max_indent_spaces_at" => Some(Self::FileMaxIndentSpacesAt),
            "file.start" => Some(Self::FileStart),
            "file.end" => Some(Self::FileEnd),
            "function.name" => Some(Self::FunctionName),
            "function.start" => Some(Self::FunctionStart),
            "function.end" => Some(Self::FunctionEnd),
            "class.name" => Some(Self::ClassName),
            "class.start" => Some(Self::ClassStart),
            "class.end" => Some(Self::ClassEnd),
            "variable.name" => Some(Self::VariableName),
            "variable.start" => Some(Self::VariableStart),
            "variable.end" => Some(Self::VariableEnd),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileMaxIndentSpacesAt => "file.max_indent_spaces_at",
            Self::FileStart => "file.start",
            Self::FileEnd => "file.end",
            Self::FunctionName => "function.name",
            Self::FunctionStart => "function.start",
            Self::FunctionEnd => "function.end",
            Self::ClassName => "class.name",
            Self::ClassStart => "class.start",
            Self::ClassEnd => "class.end",
            Self::VariableName => "variable.name",
            Self::VariableStart => "variable.start",
            Self::VariableEnd => "variable.end",
        }
    }
}

/// A diagnostic emitted by a lint rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rule_id: SmolStr,
    pub severity: Severity,
    /// Lexical scope range used for suppression evaluation.
    pub check_range: TextRange,
    /// Specific locations targeted for reporting.
    pub report_ranges: SmallVec<[TextRange; 1]>,
    pub message: String,
}

impl Diagnostic {
    /// Creates a diagnostic with a single range for both check and report.
    pub fn single_range(
        rule_id: impl Into<SmolStr>,
        severity: Severity,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            check_range: range,
            report_ranges: smallvec::smallvec![range],
            message: message.into(),
        }
    }

    /// Returns the primary reporting range (first report range, or check range if none).
    #[inline]
    pub fn primary_range(&self) -> TextRange {
        self.report_ranges
            .first()
            .copied()
            .unwrap_or(self.check_range)
    }

    /// Returns the effective slice of report ranges to render.
    #[inline]
    pub fn effective_report_ranges(&self) -> &[TextRange] {
        if self.report_ranges.is_empty() {
            std::slice::from_ref(&self.check_range)
        } else {
            &self.report_ranges
        }
    }

    /// Formats this diagnostic directly into a writer without intermediate heap allocations.
    pub fn write_formatted<W: std::fmt::Write>(
        &self,
        writer: &mut W,
        file_path: Option<&str>,
        colorize: bool,
    ) -> std::fmt::Result {
        let ranges = self.effective_report_ranges();
        if colorize {
            use common::color::{BOLD, DIM, RESET, YELLOW};
            if let Some(path) = file_path {
                write!(writer, "{path}{DIM}:{RESET}")?;
            }
            for (index, range) in ranges.iter().enumerate() {
                if index > 0 {
                    write!(writer, "{DIM},{RESET} ")?;
                }
                write!(
                    writer,
                    "{YELLOW}{}{RESET}{DIM}:{RESET}{YELLOW}{}{RESET}",
                    range.start_line, range.start_col,
                )?;
            }
            let sev_color = self.severity.color().unwrap_or(RESET);
            write!(
                writer,
                "{DIM}:{RESET} {BOLD}[{sev_color}{}{RESET}{BOLD}] {}: {}{RESET}",
                self.severity.as_str(),
                self.rule_id.as_str(),
                self.message.as_str(),
            )
        } else {
            if let Some(path) = file_path {
                write!(writer, "{path}:")?;
            }
            for (index, range) in ranges.iter().enumerate() {
                if index > 0 {
                    write!(writer, ", ")?;
                }
                write!(writer, "{}:{}", range.start_line, range.start_col)?;
            }
            write!(
                writer,
                ": [{}] {}: {}",
                self.severity.as_str(),
                self.rule_id.as_str(),
                self.message.as_str(),
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
