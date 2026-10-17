//! Tests for include graph resolution, virtual rulesets, and DAG loader invariants.

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;
use lint::rule::StatRule;
use lint::types::Scope;
use lint::{Condition, RuleRegistryBuilder};

#[test]
fn test_opt_in_ruleset_starts_with_empty_registry() {
    let yaml = r#"
lint:
  rules: []

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").expect("Valid empty config");
    assert!(registry.stat_dispatch.is_empty());
    // AST dispatch is empty when standard is not in includes
    assert!(registry.ast_dispatch.is_empty());
}

#[test]
fn test_including_standard_loads_ast_rules() {
    let yaml = r#"
lint:
  includes:
    - standard

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").expect("Valid standard include");
    // Semicolon AST rule is active
    assert!(registry.ast_dispatch.contains_key(";"));
}

#[test]
fn test_including_statistics_loads_stat_rules() {
    let yaml = r#"
lint:
  includes:
    - statistics

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").expect("Valid statistics include");
    // Function statistical rules are active
    assert!(
        registry
            .stat_dispatch
            .contains_key(&lint::types::Scope::Function)
    );
    assert!(
        registry
            .stat_dispatch
            .contains_key(&lint::types::Scope::File)
    );
}

#[test]
fn test_virtual_ruleset_memoization_preserves_overrides() {
    let yaml = r#"
lint:
  includes:
    - statistics
    - statistics
  rules:
    - id: statistics:long-function
      severity: error

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    let function_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    let rule = function_rules
        .iter()
        .find(|r| r.id == "statistics:long-function")
        .unwrap();
    assert_eq!(rule.severity, lint::types::Severity::Error);
}

#[test]
fn test_load_string_detects_basename_collision_with_included_file() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(lint::concat_str!("kost_test_collision_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let sub_file = temp_dir.join("rules.yaml");
    std::fs::write(&sub_file, "lint:\n  rules: []\n").unwrap();

    let yaml = lint::concat_str!(
        "lint:\n  includes:\n    - ",
        sub_file.to_str().unwrap(),
        "\n"
    );
    let err = ConfigLoader::load_string(&yaml, "rules.yaml").unwrap_err();
    match err {
        ConfigError::AmbiguousScope { filename, .. } => {
            assert_eq!(filename, "rules.yaml");
        }
        other => panic!("Expected AmbiguousScope, got: {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_filesystem_dag_includes_and_circular_detection() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(lint::concat_str!("kost_test_dag_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let file_a = temp_dir.join("cycle_a.yaml");
    let file_b = temp_dir.join("cycle_b.yaml");
    std::fs::write(&file_a, "lint:\n  includes:\n    - cycle_b.yaml\n").unwrap();
    std::fs::write(&file_b, "lint:\n  includes:\n    - cycle_a.yaml\n").unwrap();

    let err = ConfigLoader::load_file(&file_a).unwrap_err();
    match err {
        ConfigError::CircularInclude { chain } => {
            assert!(chain.contains("cycle_a.yaml") && chain.contains("cycle_b.yaml"));
        }
        other => panic!("Expected CircularInclude, got: {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rule_registry_builder_applies_report_id_override_to_stat_rules() {
    let mut builder = RuleRegistryBuilder::new();
    let rule = StatRule::builder("my-rule", Scope::Function)
        .condition(Condition::All(Vec::new()))
        .build();
    builder.register_stat(rule);
    builder.set_report_id("my-rule", "custom:my-rule");
    let registry = builder.build();
    let rules = registry.stat_dispatch.get(&Scope::Function).unwrap();
    assert_eq!(rules[0].report_id, "custom:my-rule");
}

#[test]
fn test_missing_lint_section_in_empty_config_rejected() {
    let err = ConfigLoader::load_string("", "empty.yaml").unwrap_err();
    match err {
        ConfigError::MissingLintSection { file } => {
            assert_eq!(file, "empty.yaml");
        }
        other => panic!("Expected MissingLintSection, got: {:?}", other),
    }

    let err_whitespace = ConfigLoader::load_string("   \n\n  ", "spaces.yaml").unwrap_err();
    match err_whitespace {
        ConfigError::MissingLintSection { file } => {
            assert_eq!(file, "spaces.yaml");
        }
        other => panic!("Expected MissingLintSection, got: {:?}", other),
    }

    let err_comment = ConfigLoader::load_string("# only a comment\n", "comment.yaml").unwrap_err();
    match err_comment {
        ConfigError::MissingLintSection { file } => {
            assert_eq!(file, "comment.yaml");
        }
        other => panic!("Expected MissingLintSection, got: {:?}", other),
    }

    let err_empty_map = ConfigLoader::load_string("{}\n", "empty_map.yaml").unwrap_err();
    match err_empty_map {
        ConfigError::MissingLintSection { file } => {
            assert_eq!(file, "empty_map.yaml");
        }
        other => panic!("Expected MissingLintSection, got: {:?}", other),
    }

    let err_null = ConfigLoader::load_string("lint: null\n", "null.yaml").unwrap_err();
    match err_null {
        ConfigError::MissingLintSection { file } => {
            assert_eq!(file, "null.yaml");
        }
        other => panic!("Expected MissingLintSection, got: {:?}", other),
    }
}

#[test]
fn test_unknown_top_level_field_rejected() {
    let yaml_legacy_includes = r#"
includes:
  - standard
"#;
    let err = ConfigLoader::load_string(yaml_legacy_includes, "kost.yaml").unwrap_err();
    match err {
        ConfigError::Yaml { source, .. } => {
            let msg = source.to_string();
            assert!(msg.contains("unknown field `includes`"));
        }
        other => panic!(
            "Expected Yaml error for unknown top-level field, got: {:?}",
            other
        ),
    }

    let yaml_legacy_rules = r#"
rules:
  - id: custom
"#;
    let err = ConfigLoader::load_string(yaml_legacy_rules, "kost.yaml").unwrap_err();
    match err {
        ConfigError::Yaml { source, .. } => {
            let msg = source.to_string();
            assert!(msg.contains("unknown field `rules`"));
        }
        other => panic!(
            "Expected Yaml error for unknown top-level field, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_unknown_field_under_lint_section_rejected() {
    let yaml = r#"
lint:
  include:
    - standard
"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::Yaml { source, .. } => {
            let msg = source.to_string();
            assert!(msg.contains("unknown field `include`"));
        }
        other => panic!(
            "Expected Yaml error for unknown field under lint:, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_unknown_field_in_rule_entry_rejected() {
    let yaml = r#"
lint:
  rules:
    - id: custom-rule
      severty: error
"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::Yaml { source, .. } => {
            let msg = source.to_string();
            assert!(msg.contains("unknown field `severty`"));
        }
        other => panic!(
            "Expected Yaml error for unknown field in rule entry, got: {:?}",
            other
        ),
    }
}
