//! Tests for rule overrides (AST and statistical rule overrides, options, and where-clause policies).

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;

#[test]
fn test_overriding_nonexistent_rule_fails() {
    let yaml = r#"
lint:
  includes:
    - standard
  rules:
    - id: standard:non-existent-rule
      severity: none

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownRuleOverride { id, .. } => {
            assert_eq!(id, "standard:non-existent-rule");
        }
        other => panic!("Expected UnknownRuleOverride, got: {:?}", other),
    }
}

#[test]
fn test_ast_rule_override_rejects_where_clause() {
    let yaml = r#"
lint:
  includes:
    - standard
  rules:
    - id: standard:no-semi
      where:
        function.lines: gt(10)

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidAstOverride { id } => {
            assert_eq!(id, "standard:no-semi");
        }
        other => panic!("Expected InvalidAstOverride, got: {:?}", other),
    }
}

#[test]
fn test_ast_rule_override_rejects_options() {
    let yaml = r#"
lint:
  includes:
    - standard
  rules:
    - id: standard:no-semi
      options:
        threshold: 10

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidAstOverride { id } => {
            assert_eq!(id, "standard:no-semi");
        }
        other => panic!("Expected InvalidAstOverride, got: {:?}", other),
    }
}

#[test]
fn test_stat_rule_override_allows_where_when_permitted() {
    let yaml = r#"
lint:
  includes:
    - statistics
  rules:
    - id: statistics:long-function
      options:
        max_lines: 80
      where:
        all:
          - function.annotations: not(has("@Composable"))
          - function.lines: gt(options.max_lines)

"#;
    let registry =
        ConfigLoader::load_string(yaml, "my_config.yaml").expect("Permitted where override");
    let function_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    let rule = function_rules
        .iter()
        .find(|r| r.id == "statistics:long-function")
        .unwrap();
    assert_eq!(rule.report_id, "my_config.yaml:long-function");
}

#[test]
fn test_stat_rule_override_rejects_undeclared_option() {
    let yaml = r#"
lint:
  includes:
    - statistics
  rules:
    - id: statistics:long-function
      options:
        unknown_option: 123

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::OptionNotDeclared {
            rule_id,
            option_key,
        } => {
            assert_eq!(rule_id, "statistics:long-function");
            assert_eq!(option_key, "unknown_option");
        }
        other => panic!("Expected OptionNotDeclared, got: {:?}", other),
    }
}

#[test]
fn test_stat_rule_override_rejects_type_mismatched_option() {
    let yaml = r#"
lint:
  includes:
    - statistics
  rules:
    - id: statistics:long-function
      options:
        max_lines: "not_a_number"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::OptionTypeMismatch {
            rule_id,
            option_key,
            expected_type,
            actual_type,
        } => {
            assert_eq!(rule_id, "statistics:long-function");
            assert_eq!(option_key, "max_lines");
            assert_eq!(expected_type, "Int");
            assert_eq!(actual_type, "Str");
        }
        other => panic!("Expected OptionTypeMismatch, got: {:?}", other),
    }
}

#[test]
fn test_override_rejects_allow_where_override() {
    let yaml = r#"
lint:
  includes:
    - statistics
  rules:
    - id: statistics:long-function
      allow_where_override: false

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::DisallowedFieldOnOverride { rule_id, field } => {
            assert_eq!(rule_id, "statistics:long-function");
            assert_eq!(field, "allow_where_override");
        }
        other => panic!("Expected DisallowedFieldOnOverride, got: {:?}", other),
    }
}
