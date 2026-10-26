//! Tests for error handling and validation during import resolution (unknown rules, empty rules, ambiguous imports, and path parsing).

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;
use lint::config::parse_qualified_import;

#[test]
fn test_fail_fast_on_unknown_rule_in_structured_import() {
    let yaml = r#"
lint:
  imports:
    - path: standard
      rules:
        - non-existent-rule

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownImportedRule { target, rule } => {
            assert_eq!(target, "standard");
            assert_eq!(rule, "non-existent-rule");
        }
        other => panic!("Expected UnknownImportedRule, got: {:?}", other),
    }
}

#[test]
fn test_fail_fast_on_unknown_rule_in_inline_import() {
    let yaml = r#"
lint:
  imports:
    - standard:unknown-rule

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownImportedRule { target, rule } => {
            assert_eq!(target, "standard");
            assert_eq!(rule, "unknown-rule");
        }
        other => panic!("Expected UnknownImportedRule, got: {:?}", other),
    }
}

#[test]
fn test_fail_fast_on_empty_rules_list() {
    let yaml = r#"
lint:
  imports:
    - path: standard
      rules: []

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::EmptyImportRules { target } => {
            assert_eq!(target, "standard");
        }
        other => panic!("Expected EmptyImportRules, got: {:?}", other),
    }
}

#[test]
fn test_inline_import_non_kebab_case_fails_with_unknown_rule() {
    let yaml = r#"
lint:
  imports:
    - standard:no_semi

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownImportedRule { target, rule } => {
            assert_eq!(target, "standard");
            assert_eq!(rule, "no_semi");
        }
        other => panic!("Expected UnknownImportedRule, got: {:?}", other),
    }
}

#[test]
fn test_ambiguous_unqualified_rule_import_fails() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_test_ambig_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let shared_yaml = temp_dir.join("shared.yaml");
    std::fs::write(
        &shared_yaml,
        r#"
lint:
  imports:
    - standard
  rules:
    - id: no-semi
      severity: error
      where:
        function.lines: gt(10)
      message: "Collision"

"#,
    )
    .unwrap();

    let main_yaml = temp_dir.join("main.yaml");
    std::fs::write(
        &main_yaml,
        r#"
lint:
  imports:
    - path: shared.yaml
      rules:
        - no-semi

"#,
    )
    .unwrap();

    let err = ConfigLoader::load_file(&main_yaml).unwrap_err();
    match err {
        ConfigError::AmbiguousImportedRule {
            target,
            rule,
            candidates,
        } => {
            assert_eq!(target, "shared.yaml");
            assert_eq!(rule, "no-semi");
            assert!(candidates.contains("standard:no-semi"));
            assert!(candidates.contains("shared.yaml:no-semi"));
        }
        other => panic!("Expected AmbiguousImportedRule, got: {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_qualified_import_parser_distinguishes_windows_paths_and_rules() {
    assert_eq!(parse_qualified_import(r"C:\rules\shared.yaml"), None);
    assert_eq!(parse_qualified_import("C:/rules/shared.yaml"), None);
    assert_eq!(
        parse_qualified_import(r"C:\rules\shared.yaml:my-rule"),
        Some((r"C:\rules\shared.yaml", "my-rule"))
    );
    assert_eq!(
        parse_qualified_import("C:/rules/shared.yaml:my-rule"),
        Some(("C:/rules/shared.yaml", "my-rule"))
    );
    assert_eq!(
        parse_qualified_import("standard:no-semi"),
        Some(("standard", "no-semi"))
    );
    assert_eq!(parse_qualified_import("shared.yaml"), None);
    assert_eq!(parse_qualified_import(":"), None);
    assert_eq!(parse_qualified_import("standard:"), None);
    assert_eq!(parse_qualified_import(":no-semi"), None);
}
