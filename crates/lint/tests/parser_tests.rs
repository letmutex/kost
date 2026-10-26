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
fn test_condition_rejects_incompatible_scope_mixing() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      where:
        all:
          - function.lines: gt(10)
          - import.is_wildcard: eq(true)
      message: "Incompatible"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::ScopeMismatch {
            rule_id,
            expected_scope,
            actual_scope,
            property,
        } => {
            assert_eq!(rule_id, "kost.yaml:custom-rule");
            assert_eq!(expected_scope, "function");
            assert_eq!(actual_scope, "import");
            assert_eq!(property, "is_wildcard");
        }
        other => panic!("Expected ScopeMismatch, got: {:?}", other),
    }
}

#[test]
fn test_condition_identifies_incompatible_scope_in_multi_scope_rule() {
    let yaml = r#"
lint:
  rules:
    - id: multi-scope-rule
      where:
        all:
          - class.lines: gt(50)
          - function.lines: gt(10)
          - import.is_wildcard: eq(true)
      message: "Incompatible"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::ScopeMismatch {
            rule_id,
            actual_scope,
            property,
            ..
        } => {
            assert_eq!(rule_id, "kost.yaml:multi-scope-rule");
            assert_eq!(actual_scope, "import");
            assert_eq!(property, "is_wildcard");
        }
        other => panic!("Expected ScopeMismatch, got: {:?}", other),
    }
}

#[test]
fn test_condition_rejects_deferred_file_aggregate_in_child_scope() {
    let yaml = r#"
lint:
  rules:
    - id: mixed-deferred-rule
      where:
        all:
          - file.functions_count: gt(5)
          - function.lines: gt(10)
      message: "Invalid deferred mixing"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::DeferredPropertyInChildScope { rule_id, property } => {
            assert_eq!(rule_id, "kost.yaml:mixed-deferred-rule");
            assert_eq!(property, "functions_count");
        }
        other => panic!("Expected DeferredPropertyInChildScope, got: {:?}", other),
    }
}

#[test]
fn test_condition_allows_deferred_file_aggregate_in_file_scope() {
    let yaml = r#"
lint:
  rules:
    - id: file-functions-count-rule
      where:
        file.functions_count: gt(5)
      message: "Too many functions: ${file.functions_count}"

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").expect("Valid file rule");
    let file_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::File)
        .expect("Dispatched to File scope");
    assert_eq!(file_rules.len(), 1);
    assert_eq!(file_rules[0].id, "kost.yaml:file-functions-count-rule");
}

#[test]
fn test_message_placeholder_rejects_deferred_file_aggregate_in_child_scope() {
    let yaml = r#"
lint:
  rules:
    - id: mixed-rule
      where:
        all:
          - file.lines: gt(10)
          - function.lines: gt(20)
      message: "Function ${function.name} in file with ${file.functions_count} functions"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidPlaceholder {
            placeholder,
            reason,
        } => {
            assert_eq!(placeholder, "file.functions_count");
            assert!(reason.contains("Deferred file aggregate property"));
        }
        other => panic!("Expected InvalidPlaceholder, got: {:?}", other),
    }
}

#[test]
fn test_condition_supports_hierarchical_scope_mixing() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      where:
        all:
          - function.lines: gt(10)
          - class.lines: gt(50)
      message: "Function ${function.name} in class ${class.name}"

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").expect("Valid mixed scope rule");
    let function_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .expect("Dispatched to Function scope");
    assert_eq!(function_rules.len(), 1);
    assert_eq!(function_rules[0].id, "kost.yaml:custom-rule");
    assert_eq!(function_rules[0].scope, lint::types::Scope::Function);
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
