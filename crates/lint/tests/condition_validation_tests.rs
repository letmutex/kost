//! Tests for condition validation in `where` clauses showing rule ID.

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;

#[test]
fn test_unknown_scope_in_where_shows_rule_id() {
    let yaml = r#"
lint:
  rules:
    - id: bad-scope-rule
      where:
        unknown_scope.lines: gt(10)
      message: "Error"
"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match &err {
        ConfigError::UnknownScope {
            rule_id,
            scope,
            property,
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-scope-rule");
            assert_eq!(scope, "unknown_scope");
            assert_eq!(property, "unknown_scope.lines");
        }
        other => panic!("Expected UnknownScope, got: {:?}", other),
    }
    let error_message = err.to_string();
    assert!(error_message.contains("kost.yaml:bad-scope-rule"));
}

#[test]
fn test_unknown_property_in_where_shows_rule_id() {
    let yaml = r#"
lint:
  rules:
    - id: bad-prop-rule
      where:
        function.nonexistent: gt(10)
      message: "Error"
"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match &err {
        ConfigError::UnknownProperty {
            rule_id,
            scope,
            property,
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-prop-rule");
            assert_eq!(scope, "function");
            assert_eq!(property, "nonexistent");
        }
        other => panic!("Expected UnknownProperty, got: {:?}", other),
    }
    let error_message = err.to_string();
    assert!(error_message.contains("kost.yaml:bad-prop-rule"));
}

#[test]
fn test_startswith_operator_incompatible_property_type_rejected() {
    let startswith_yaml = r#"
lint:
  rules:
    - id: bad-startswith
      where:
        file.lines: startswith("1")
      message: "Error"
"#;
    let err = ConfigLoader::load_string(startswith_yaml, "kost.yaml").unwrap_err();
    let error_message = err.to_string();
    match err {
        ConfigError::IncompatiblePropertyOperator {
            rule_id,
            property,
            property_type,
            operator,
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-startswith");
            assert_eq!(property, "file.lines");
            assert_eq!(property_type, "Int");
            assert_eq!(operator, "startswith");
        }
        other => panic!(
            "Expected IncompatiblePropertyOperator for startswith, got: {:?}",
            other
        ),
    }
    assert!(error_message.contains("kost.yaml:bad-startswith"));
}

#[test]
fn test_endswith_operator_incompatible_property_type_rejected() {
    let endswith_yaml = r#"
lint:
  rules:
    - id: bad-endswith
      where:
        function.lines: endswith("0")
      message: "Error"
"#;
    let err = ConfigLoader::load_string(endswith_yaml, "kost.yaml").unwrap_err();
    let error_message = err.to_string();
    match err {
        ConfigError::IncompatiblePropertyOperator {
            rule_id,
            property,
            property_type,
            operator,
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-endswith");
            assert_eq!(property, "function.lines");
            assert_eq!(property_type, "Int");
            assert_eq!(operator, "endswith");
        }
        other => panic!(
            "Expected IncompatiblePropertyOperator for endswith, got: {:?}",
            other
        ),
    }
    assert!(error_message.contains("kost.yaml:bad-endswith"));
}

#[test]
fn test_has_operator_incompatible_property_type_rejected() {
    let has_yaml = r#"
lint:
  rules:
    - id: bad-has
      where:
        file.lines: has("1")
      message: "Error"
"#;
    let err = ConfigLoader::load_string(has_yaml, "kost.yaml").unwrap_err();
    let error_message = err.to_string();
    match err {
        ConfigError::IncompatiblePropertyOperator {
            rule_id,
            property,
            property_type,
            operator,
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-has");
            assert_eq!(property, "file.lines");
            assert_eq!(property_type, "Int");
            assert_eq!(operator, "has");
        }
        other => panic!(
            "Expected IncompatiblePropertyOperator for has, got: {:?}",
            other
        ),
    }
    assert!(error_message.contains("kost.yaml:bad-has"));
}

#[test]
fn test_empty_option_key_rejected() {
    let empty_key_yaml = r#"
lint:
  rules:
    - id: bad-empty-option-key
      options:
        "": 10
      where:
        file.lines: gt(10)
      message: "Error"
"#;
    let err = ConfigLoader::load_string(empty_key_yaml, "kost.yaml").unwrap_err();
    let error_message = err.to_string();
    match err {
        ConfigError::InvalidOptionKey {
            rule_id,
            option_key,
            ..
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-empty-option-key");
            assert_eq!(option_key, "");
        }
        other => panic!("Expected InvalidOptionKey, got: {:?}", other),
    }
    assert!(error_message.contains("kost.yaml:bad-empty-option-key"));
}

#[test]
fn test_invalid_option_key_syntax_rejected() {
    let invalid_key_yaml = r#"
lint:
  rules:
    - id: bad-option-key
      options:
        "invalid$key": 10
      where:
        file.lines: gt(10)
      message: "Error"
"#;
    let err = ConfigLoader::load_string(invalid_key_yaml, "kost.yaml").unwrap_err();
    let error_message = err.to_string();
    match err {
        ConfigError::InvalidOptionKey {
            rule_id,
            option_key,
            ..
        } => {
            assert_eq!(rule_id, "kost.yaml:bad-option-key");
            assert_eq!(option_key, "invalid$key");
        }
        other => panic!("Expected InvalidOptionKey, got: {:?}", other),
    }
    assert!(error_message.contains("kost.yaml:bad-option-key"));
}

#[test]
fn test_report_at_unknown_target_rejected() {
    let yaml = r#"
lint:
  rules:
    - id: bad-target
      where:
        function.lines: gt(50)
      report_at: function.unknown_anchor
      message: "Error"
"#;
    let err = ConfigLoader::load_string(yaml, "test.yaml").unwrap_err();
    match err {
        ConfigError::UnknownReportTarget { rule_id, target } => {
            assert_eq!(rule_id, "test.yaml:bad-target");
            assert_eq!(target, "function.unknown_anchor");
        }
        other => panic!("Expected UnknownReportTarget, got: {other:?}"),
    }
}

#[test]
fn test_report_at_scope_mismatch_rejected() {
    let yaml = r#"
lint:
  rules:
    - id: mismatched-scope
      where:
        file.max_indent_spaces: gt(8)
      report_at: function.name
      message: "Error"
"#;
    let err = ConfigLoader::load_string(yaml, "test.yaml").unwrap_err();
    match err {
        ConfigError::InvalidReportTargetScope {
            rule_id,
            target,
            target_scope,
        } => {
            assert_eq!(rule_id, "test.yaml:mismatched-scope");
            assert_eq!(target, "function.name");
            assert_eq!(target_scope, "function");
        }
        other => panic!("Expected InvalidReportTargetScope, got: {other:?}"),
    }
}

#[test]
fn test_report_at_multi_target_execution_and_format() {
    let yaml = r#"
lint:
  rules:
    - id: multiline-func
      where:
        function.lines: gt(1)
      report_at:
        - function.start
        - function.end
      message: "Multiline function"
"#;
    let registry = ConfigLoader::load_string(yaml, "custom.yaml").unwrap();
    let code = "fun hello() {\n    val a = 1\n}\n";
    let diags = lint::walker::lint_source(code.as_bytes(), &registry);
    assert_eq!(diags.len(), 1);
    let diag = &diags[0];
    assert_eq!(diag.report_ranges.len(), 2);
    assert_eq!(diag.report_ranges[0].start_line, 1);
    assert_eq!(diag.report_ranges[0].start_col, 1);
    assert_eq!(diag.report_ranges[1].start_line, 3);
    assert_eq!(diag.report_ranges[1].start_col, 2);
    assert_eq!(
        diag.to_string(),
        "1:1, 3:2: [warning] custom.yaml:multiline-func: Multiline function"
    );
}
