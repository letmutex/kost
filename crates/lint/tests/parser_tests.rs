//! Unit and integration tests for strict rule parsing and includes resolution.

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;
use lint::types::{Severity, WherePolicy};

#[test]
fn test_custom_rule_id_rejects_colon() {
    let yaml = r#"
lint:
  rules:
    - id: custom:rule-name
      where:
        function.lines: gt(10)
      message: "Too long"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownRuleOverride { id, .. } => {
            assert_eq!(id, "custom:rule-name");
        }
        other => panic!("Expected UnknownRuleOverride, got: {:?}", other),
    }
}

#[test]
fn test_custom_rule_id_rejects_non_kebab_case() {
    let yaml = r#"
lint:
  rules:
    - id: Invalid_Name!
      where:
        function.lines: gt(10)
      message: "Too long"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidIdFormat { id, reason } => {
            assert_eq!(id, "Invalid_Name!");
            assert!(reason.contains("kebab-case"));
        }
        other => panic!("Expected InvalidIdFormat, got: {:?}", other),
    }
}

#[test]
fn test_custom_rule_binds_filename_as_scope() {
    let yaml = r#"
lint:
  rules:
    - id: function-too-long
      severity: error
      options:
        threshold: 50
      where:
        function.lines: gt(options.threshold)
      message: "Function is too long (${function.lines} > ${options.threshold})"

"#;
    let registry = ConfigLoader::load_string(yaml, "team_rules.yaml").expect("Valid custom rule");
    let function_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    let rule = function_rules
        .iter()
        .find(|r| r.id == "team_rules.yaml:function-too-long")
        .unwrap();
    assert_eq!(rule.report_id, "team_rules.yaml:function-too-long");
    assert_eq!(rule.severity, Severity::Error);
    assert_eq!(rule.where_policy, WherePolicy::AllowOverride);
}

#[test]
fn test_empty_options_block_is_rejected() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      options: {}
      where:
        function.lines: gt(10)
      message: "Failed"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::EmptyOptions { rule_id } => {
            assert_eq!(rule_id, "kost.yaml:custom-rule");
        }
        other => panic!("Expected EmptyOptions, got: {:?}", other),
    }
}

#[test]
fn test_message_placeholder_validation_rejects_unknown_property() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      where:
        function.lines: gt(10)
      message: "Function has ${function.invalid_property} lines"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidPlaceholder { placeholder, .. } => {
            assert_eq!(placeholder, "function.invalid_property");
        }
        other => panic!("Expected InvalidPlaceholder, got: {:?}", other),
    }
}

#[test]
fn test_message_placeholder_validation_rejects_scope_mismatch() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      where:
        function.lines: gt(10)
      message: "Class name is ${class.name}"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidPlaceholder { placeholder, .. } => {
            assert_eq!(placeholder, "class.name");
        }
        other => panic!("Expected InvalidPlaceholder, got: {:?}", other),
    }
}

#[test]
fn test_condition_rejects_scope_mixing() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      where:
        all:
          - function.lines: gt(10)
          - class.lines: gt(50)
      message: "Mixed"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::ScopeMismatch {
            rule_id,
            expected_scope,
            actual_scope,
            ..
        } => {
            assert_eq!(rule_id, "kost.yaml:custom-rule");
            assert_eq!(expected_scope, "function");
            assert_eq!(actual_scope, "class");
        }
        other => panic!("Expected ScopeMismatch, got: {:?}", other),
    }
}

#[test]
fn test_undeclared_options_reference_rejected_when_rule_has_no_options() {
    let yaml = r#"
lint:
  rules:
    - id: custom-check
      where:
        function.lines: gt(options.threshold)
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::OptionNotDeclared {
            rule_id,
            option_key,
        } => {
            assert_eq!(rule_id, "kost.yaml:custom-check");
            assert_eq!(option_key, "threshold");
        }
        other => panic!("Expected OptionNotDeclared, got: {:?}", other),
    }
}
