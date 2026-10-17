//! Error types for configuration parsing, validation, and include resolution.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid rule ID '{id}': {reason}")]
    InvalidIdFormat { id: String, reason: &'static str },

    #[error(
        "Rule '{id}' cannot be overridden in '{file}': rule does not exist in any included ruleset"
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

    #[error(
        "Option '{option_key}' on rule '{rule_id}' expected type '{expected_type}', found '{actual_type}'"
    )]
    OptionTypeMismatch {
        rule_id: String,
        option_key: String,
        expected_type: &'static str,
        actual_type: &'static str,
    },

    #[error("Unknown scope '{scope}' in property '{property}'")]
    UnknownScope { scope: String, property: String },

    #[error("Unknown property '{property}' for scope '{scope}'")]
    UnknownProperty { scope: String, property: String },

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
        "Invalid severity '{severity}' for rule '{rule_id}': expected 'error', 'warning', or 'none'"
    )]
    InvalidSeverity { rule_id: String, severity: String },

    #[error(
        "Operator '{operator}' is incompatible with property '{property}' of type {property_type}"
    )]
    IncompatiblePropertyOperator {
        property: String,
        property_type: &'static str,
        operator: &'static str,
    },

    #[error("Invalid condition operator '{operator}' for property '{property}': {reason}")]
    InvalidOperator {
        property: String,
        operator: String,
        reason: &'static str,
    },

    #[error(
        "Invalid operand for operator '{operator}' on property '{property}': expected {expected}, found {actual}"
    )]
    InvalidOperandType {
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

    #[error("Invalid placeholder '${{{placeholder}}}' in message template: {reason}")]
    InvalidPlaceholder {
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

    #[error("Circular include detected: {chain}")]
    CircularInclude { chain: String },

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
    UnknownIncludedRule { target: String, rule: String },

    #[error("Empty 'rules: []' selection list for included target '{target}'")]
    EmptyIncludeRules { target: String },

    #[error(
        "Rule '{rule}' in target '{target}' is ambiguous: matches multiple exported rules: {candidates}. Use a fully-qualified rule ID instead"
    )]
    AmbiguousIncludedRule {
        target: String,
        rule: String,
        candidates: String,
    },
}
