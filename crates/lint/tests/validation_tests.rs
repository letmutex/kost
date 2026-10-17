//! Validation tests for condition structure, operator type checking, and placeholder syntax.

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;

#[test]
fn test_where_clause_rejects_multiple_top_level_conditions() {
    let yaml = r#"
lint:
  rules:
    - id: multi-top-level
      where:
        function.lines: gt(50)
        function.name: eq("foo")
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidOperator {
            property, reason, ..
        } => {
            assert_eq!(property, "where");
            assert!(reason.contains("exactly one combinator"));
        }
        other => panic!(
            "Expected InvalidOperator for multi-top-level, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_property_and_operator_type_compatibility() {
    let yaml = r#"
lint:
  rules:
    - id: invalid-type-op
      where:
        function.name: gt(10)
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::IncompatiblePropertyOperator {
            property,
            property_type,
            operator,
        } => {
            assert_eq!(property, "function.name");
            assert_eq!(property_type, "Str");
            assert_eq!(operator, "gt");
        }
        other => panic!("Expected IncompatiblePropertyOperator, got: {:?}", other),
    }
}

#[test]
fn test_invalid_severity_string_fails_fast() {
    let yaml = r#"
lint:
  rules:
    - id: typo-severity
      severity: errror
      where:
        function.lines: gt(10)
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidSeverity { rule_id, severity } => {
            assert_eq!(rule_id, "kost.yaml:typo-severity");
            assert_eq!(severity, "errror");
        }
        other => panic!("Expected InvalidSeverity, got: {:?}", other),
    }
}

#[test]
fn test_unclosed_placeholder_in_message_template_is_rejected() {
    let yaml = r#"
lint:
  rules:
    - id: unclosed-placeholder
      where:
        function.lines: gt(10)
      message: "Function ${function.name is too long"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidPlaceholder { reason, .. } => {
            assert!(reason.contains("Unclosed placeholder"));
        }
        other => panic!("Expected InvalidPlaceholder, got: {:?}", other),
    }
}

#[test]
fn test_eq_operator_rejects_type_mismatched_operand() {
    let yaml = r#"
lint:
  rules:
    - id: mismatched-eq
      where:
        function.lines: eq("ten")
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidOperandType {
            property,
            operator,
            expected,
            actual,
        } => {
            assert_eq!(property, "function.lines");
            assert_eq!(operator, "eq");
            assert_eq!(expected, "Int");
            assert_eq!(actual, "Str");
        }
        other => panic!("Expected InvalidOperandType for eq, got: {:?}", other),
    }
}

#[test]
fn test_option_rejects_nested_mapping_or_unsupported_structure() {
    let yaml = r#"
lint:
  rules:
    - id: bad-option
      options:
        threshold:
          nested: 10
      where:
        function.lines: gt(10)
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidOperandType {
            property, operator, ..
        } => {
            assert_eq!(property, "threshold");
            assert_eq!(operator, "options");
        }
        other => panic!(
            "Expected InvalidOperandType for nested option, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_where_clause_rejects_empty_combinator_sequence() {
    let yaml = r#"
lint:
  rules:
    - id: empty-all
      where:
        all: []
      message: "Error"

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidOperator {
            property,
            operator,
            reason,
        } => {
            assert_eq!(property, "where");
            assert_eq!(operator, "combinator");
            assert!(reason.contains("empty"));
        }
        other => panic!(
            "Expected InvalidOperator for empty combinator, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_option_override_rejects_invalid_regex_pattern() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(lint::concat_str!("kost_test_regex_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let base_file = temp_dir.join("base.yaml");
    let base_yaml = r#"
lint:
  rules:
    - id: custom-rule
      options:
        pattern: "^[a-z]+$"
      where:
        function.name: matches(options.pattern)
      message: "Error"

"#;
    std::fs::write(&base_file, base_yaml).unwrap();

    let override_file = temp_dir.join("override.yaml");
    let override_yaml = r#"
lint:
  includes:
    - base.yaml
  rules:
    - id: base.yaml:custom-rule
      options:
        pattern: "[invalid regex"

"#;
    std::fs::write(&override_file, override_yaml).unwrap();

    let err = ConfigLoader::load_file(&override_file).unwrap_err();
    match err {
        ConfigError::InvalidOperator {
            property,
            operator,
            reason,
        } => {
            assert_eq!(property, "pattern");
            assert_eq!(operator, "matches");
            assert!(reason.contains("regular expression"));
        }
        other => panic!(
            "Expected InvalidOperator for bad regex pattern option override, got: {:?}",
            other
        ),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
