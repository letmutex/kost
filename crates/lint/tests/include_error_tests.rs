//! Tests for error handling and validation during include resolution (unknown rules, empty rules, ambiguous imports, and path parsing).

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;
use lint::config::parse_qualified_include;

#[test]
fn test_fail_fast_on_unknown_rule_in_structured_include() {
    let yaml = r#"
lint:
  includes:
    - path: standard
      rules:
        - non-existent-rule

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownIncludedRule { target, rule } => {
            assert_eq!(target, "standard");
            assert_eq!(rule, "non-existent-rule");
        }
        other => panic!("Expected UnknownIncludedRule, got: {:?}", other),
    }
}

#[test]
fn test_fail_fast_on_unknown_rule_in_inline_include() {
    let yaml = r#"
lint:
  includes:
    - standard:unknown-rule

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownIncludedRule { target, rule } => {
            assert_eq!(target, "standard");
            assert_eq!(rule, "unknown-rule");
        }
        other => panic!("Expected UnknownIncludedRule, got: {:?}", other),
    }
}

#[test]
fn test_fail_fast_on_empty_rules_list() {
    let yaml = r#"
lint:
  includes:
    - path: standard
      rules: []

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::EmptyIncludeRules { target } => {
            assert_eq!(target, "standard");
        }
        other => panic!("Expected EmptyIncludeRules, got: {:?}", other),
    }
}

#[test]
fn test_inline_include_non_kebab_case_fails_with_unknown_rule() {
    let yaml = r#"
lint:
  includes:
    - standard:no_semi

"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::UnknownIncludedRule { target, rule } => {
            assert_eq!(target, "standard");
            assert_eq!(rule, "no_semi");
        }
        other => panic!("Expected UnknownIncludedRule, got: {:?}", other),
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
  includes:
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
  includes:
    - path: shared.yaml
      rules:
        - no-semi

"#,
    )
    .unwrap();

    let err = ConfigLoader::load_file(&main_yaml).unwrap_err();
    match err {
        ConfigError::AmbiguousIncludedRule {
            target,
            rule,
            candidates,
        } => {
            assert_eq!(target, "shared.yaml");
            assert_eq!(rule, "no-semi");
            assert!(candidates.contains("standard:no-semi"));
            assert!(candidates.contains("shared.yaml:no-semi"));
        }
        other => panic!("Expected AmbiguousIncludedRule, got: {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_qualified_include_parser_distinguishes_windows_paths_and_rules() {
    assert_eq!(parse_qualified_include(r"C:\rules\shared.yaml"), None);
    assert_eq!(parse_qualified_include("C:/rules/shared.yaml"), None);
    assert_eq!(
        parse_qualified_include(r"C:\rules\shared.yaml:my-rule"),
        Some((r"C:\rules\shared.yaml", "my-rule"))
    );
    assert_eq!(
        parse_qualified_include("C:/rules/shared.yaml:my-rule"),
        Some(("C:/rules/shared.yaml", "my-rule"))
    );
    assert_eq!(
        parse_qualified_include("standard:no-semi"),
        Some(("standard", "no-semi"))
    );
    assert_eq!(parse_qualified_include("shared.yaml"), None);
    assert_eq!(parse_qualified_include(":"), None);
    assert_eq!(parse_qualified_include("standard:"), None);
    assert_eq!(parse_qualified_include(":no-semi"), None);
}
