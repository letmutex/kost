//! Strongly-typed, filesystem-free Kotlin linting engine facade.

use crate::config::ConfigLoader;
use crate::config::error::ConfigError;
use crate::registry::RuleRegistry;
use crate::types::{Diagnostic, Severity};
use std::sync::Arc;

/// An immutable, thread-safe Kotlin static analysis engine.
///
/// `Linter` is completely decoupled from the filesystem and can be safely shared
/// across threads via its internal `Arc<RuleRegistry>`.
#[derive(Clone)]
pub struct Linter {
    registry: Arc<RuleRegistry>,
}

impl Linter {
    /// Constructs a linter with the standard AST ruleset enabled.
    ///
    /// This constructor is 100% in-memory and filesystem-free.
    pub fn standard() -> Self {
        let registry = ConfigLoader::load_string(
            "lint:\n  includes:\n    - standard\n",
            "virtual:standard.yaml",
        )
        .expect("Virtual standard ruleset must always construct cleanly");
        Self {
            registry: Arc::new(registry),
        }
    }

    /// Constructs a linter from a list of ruleset names (e.g. `&["standard"]` or `&["standard", "statistics"]`).
    ///
    /// When referencing built-in virtual rulesets (`standard`, `statistics`), this constructor
    /// is 100% in-memory and filesystem-free.
    pub fn from_rulesets(rulesets: &[&str]) -> Result<Self, ConfigError> {
        let valid_rulesets: Vec<&str> = rulesets
            .iter()
            .copied()
            .map(str::trim)
            .filter(|ruleset| !ruleset.is_empty())
            .collect();

        if valid_rulesets.is_empty() {
            return Ok(Self::standard());
        }

        let mut yaml = String::from("lint:\n  includes:\n");
        for ruleset in valid_rulesets {
            let sanitized = ruleset
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            yaml.push_str("    - \"");
            yaml.push_str(&sanitized);
            yaml.push_str("\"\n");
        }

        Self::from_yaml(&yaml)
    }

    /// Constructs a linter by parsing an in-memory YAML configuration string.
    ///
    /// This constructor is 100% in-memory and filesystem-free.
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        let registry = ConfigLoader::load_string(yaml, "virtual:inline.yaml")?;
        Ok(Self {
            registry: Arc::new(registry),
        })
    }

    /// Constructs a linter directly from a pre-compiled `RuleRegistry`.
    pub fn from_registry(registry: RuleRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }

    /// Returns a reference to the active `RuleRegistry`.
    pub fn registry(&self) -> &RuleRegistry {
        &self.registry
    }

    /// Lints source bytes without an explicit file path.
    pub fn lint_source(&self, source: &[u8]) -> LintReport {
        self.lint_source_with_path(source, None)
    }

    /// Lints source bytes with an optional virtual file path context.
    pub fn lint_source_with_path(&self, source: &[u8], file_path: Option<&str>) -> LintReport {
        self.lint_source_with_config(source, file_path, None)
    }

    /// Lints source bytes with optional virtual file path and resolved EditorConfig settings.
    pub fn lint_source_with_config(
        &self,
        source: &[u8],
        file_path: Option<&str>,
        editorconfig: Option<&common::editorconfig::ResolvedEditorConfig>,
    ) -> LintReport {
        let diagnostics = crate::walker::lint_source_with_path_and_config(
            source,
            file_path,
            &self.registry,
            editorconfig,
        );
        let mut report = LintReport::new(diagnostics, file_path.map(|path| path.to_string()));
        if !report.is_clean()
            && let Ok(utf8_str) = std::str::from_utf8(source)
        {
            report = report.with_source(utf8_str);
        }
        report
    }

    /// Lints a UTF-8 source string without an explicit file path.
    pub fn lint_str(&self, source: &str) -> LintReport {
        self.lint_source(source.as_bytes())
    }

    /// Lints a UTF-8 source string with an explicit virtual file path context.
    pub fn lint_str_with_path(&self, source: &str, file_path: &str) -> LintReport {
        self.lint_source_with_path(source.as_bytes(), Some(file_path))
    }

    /// Lints a UTF-8 source string with explicit virtual file path and EditorConfig settings.
    pub fn lint_str_with_config(
        &self,
        source: &str,
        file_path: Option<&str>,
        editorconfig: Option<&common::editorconfig::ResolvedEditorConfig>,
    ) -> LintReport {
        self.lint_source_with_config(source.as_bytes(), file_path, editorconfig)
    }
}

/// A structured evaluation report containing all emitted diagnostics for a linted unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintReport {
    diagnostics: Vec<Diagnostic>,
    file_path: Option<String>,
    source: Option<std::sync::Arc<str>>,
    error_count: usize,
    warning_count: usize,
}

impl LintReport {
    /// Creates a new `LintReport` from diagnostics and an optional virtual file path.
    pub fn new(mut diagnostics: Vec<Diagnostic>, file_path: Option<String>) -> Self {
        diagnostics.sort_by(|left, right| {
            left.range
                .start_line
                .cmp(&right.range.start_line)
                .then_with(|| left.range.start_col.cmp(&right.range.start_col))
                .then_with(|| left.rule_id.cmp(&right.rule_id))
        });

        let mut error_count = 0;
        let mut warning_count = 0;
        for diagnostic in &diagnostics {
            match diagnostic.severity {
                Severity::Error => error_count += 1,
                Severity::Warning => warning_count += 1,
                Severity::None => {}
            }
        }
        Self {
            diagnostics,
            file_path,
            source: None,
            error_count,
            warning_count,
        }
    }

    /// Attaches the original source code to this report for rich diagnostic printing.
    pub fn with_source(mut self, source: impl Into<std::sync::Arc<str>>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Returns a reference to the source code associated with this report, if available.
    #[inline]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Returns true if no diagnostics were produced.
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns true if at least one diagnostic has `Severity::Error`.
    #[inline]
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    /// Returns true if at least one diagnostic has `Severity::Warning`.
    #[inline]
    pub fn has_warnings(&self) -> bool {
        self.warning_count > 0
    }

    /// Returns the total count of error-level diagnostics.
    #[inline]
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Returns the total count of warning-level diagnostics.
    #[inline]
    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    /// Returns a borrowed slice of all emitted diagnostics.
    #[inline]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consumes the report and returns the underlying diagnostics vector.
    #[inline]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Returns the virtual or physical file path associated with this report, if any.
    #[inline]
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }
}

impl std::fmt::Display for LintReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.file_path.as_deref();
        for diagnostic in &self.diagnostics {
            diagnostic.write_formatted(formatter, path, false)?;
            writeln!(formatter)?;
        }
        Ok(())
    }
}
