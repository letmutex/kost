//! Error types for configuration parsing, validation, and import resolution.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid rule ID '{id}': {reason}")]
    InvalidIdFormat { id: String, reason: &'static str },

    #[error(
        "Rule '{id}' cannot be overridden in '{file}': rule does not exist in any imported ruleset"
    )]
    UnknownRuleOverride { id: String, file: String },

    #[error("Cannot override '{id}': AST rules only support 'severity' overrides")]
    InvalidAstOverride { id: String },

    #[error(
        "Cannot override 'where' on '{id}': rule has locked conditions (allow_where_override: false); configure 'options' or 'severity' instead"
    )]
    WhereOverrideRejected { id: String },

    #[error(
        "Rule '{rule_id}' defines an empty 'options: {{}}' block; options block must contain at least one key-value pair"
    )]
    EmptyOptions { rule_id: String },

    #[error("Option '{option_key}' is not declared by rule '{rule_id}'")]
    OptionNotDeclared { rule_id: String, option_key: String },

    #[error("Invalid option key '{option_key}' in rule '{rule_id}': {reason}")]
    InvalidOptionKey {
        rule_id: String,
        option_key: String,
        reason: &'static str,
    },

    #[error(
        "Option '{option_key}' on rule '{rule_id}' expected type '{expected_type}', found '{actual_type}'"
    )]
    OptionTypeMismatch {
        rule_id: String,
        option_key: String,
        expected_type: &'static str,
        actual_type: &'static str,
    },

    #[error("Unknown scope '{scope}' in property '{property}' in rule '{rule_id}'")]
    UnknownScope {
        rule_id: String,
        scope: String,
        property: String,
    },

    #[error("Unknown property '{property}' for scope '{scope}' in rule '{rule_id}'")]
    UnknownProperty {
        rule_id: String,
        scope: String,
        property: String,
    },

    #[error(
        "Scope mismatch in rule '{rule_id}': rule scope is '{expected_scope}', but property targets '{actual_scope}'"
    )]
    ScopeMismatch {
        rule_id: String,
        expected_scope: &'static str,
        actual_scope: &'static str,
        property: String,
    },

    #[error(
        "Deferred property '{property}' in rule '{rule_id}' is aggregated across the entire file and can only be evaluated in file-scoped rules"
    )]
    DeferredPropertyInChildScope {
        rule_id: String,
        property: &'static str,
    },

    #[error("Unknown report target '{target}' in rule '{rule_id}'")]
    UnknownReportTarget { rule_id: String, target: String },

    #[error(
        "Report target '{target}' (scope '{target_scope}') in rule '{rule_id}' is forbidden: rule condition has no '{target_scope}' scope"
    )]
    InvalidReportTargetScope {
        rule_id: String,
        target: String,
        target_scope: &'static str,
    },

    #[error(
        "Invalid severity '{severity}' for rule '{rule_id}': expected 'error', 'warning', or 'none'"
    )]
    InvalidSeverity { rule_id: String, severity: String },

    #[error(
        "Operator '{operator}' in rule '{rule_id}' is incompatible with property '{property}' of type {property_type}"
    )]
    IncompatiblePropertyOperator {
        rule_id: String,
        property: String,
        property_type: &'static str,
        operator: &'static str,
    },

    #[error(
        "Invalid condition operator '{operator}' for property '{property}' in rule '{rule_id}': {reason}"
    )]
    InvalidOperator {
        rule_id: String,
        property: String,
        operator: String,
        reason: &'static str,
    },

    #[error(
        "Invalid operand for operator '{operator}' on property '{property}' in rule '{rule_id}': expected {expected}, found {actual}"
    )]
    InvalidOperandType {
        rule_id: String,
        property: String,
        operator: &'static str,
        expected: &'static str,
        actual: String,
    },

    #[error("Field '{field}' is not allowed on overriding rule '{rule_id}'")]
    DisallowedFieldOnOverride {
        rule_id: String,
        field: &'static str,
    },

    #[error(
        "Invalid placeholder '${{{placeholder}}}' in message template for rule '{rule_id}': {reason}"
    )]
    InvalidPlaceholder {
        rule_id: String,
        placeholder: String,
        reason: &'static str,
    },

    #[error("Missing required field '{field}' for rule '{rule_id}'")]
    MissingRequiredField {
        rule_id: String,
        field: &'static str,
    },

    #[error("Missing required 'lint:' section in configuration file '{file}'")]
    MissingLintSection { file: String },

    #[error("Circular import detected: {chain}")]
    CircularImport { chain: String },

    #[error(
        "Ambiguous rule scope '{filename}': both '{first_path}' and '{second_path}' share this filename. Rename one configuration file to ensure distinct rule scopes"
    )]
    AmbiguousScope {
        filename: String,
        first_path: String,
        second_path: String,
    },

    #[error("YAML parse error in '{file}': {source}")]
    Yaml {
        file: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("Failed to read file '{file}': {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Rule '{rule}' is not defined in target '{target}'")]
    UnknownImportedRule { target: String, rule: String },

    #[error("Empty 'rules: []' selection list for imported target '{target}'")]
    EmptyImportRules { target: String },

    #[error(
        "Rule '{rule}' in target '{target}' is ambiguous: matches multiple exported rules: {candidates}. Use a fully-qualified rule ID instead"
    )]
    AmbiguousImportedRule {
        target: String,
        rule: String,
        candidates: String,
    },

    #[error("Invalid glob pattern in lint.ignore '{pattern}': {reason}")]
    InvalidIgnorePattern { pattern: String, reason: String },
}
