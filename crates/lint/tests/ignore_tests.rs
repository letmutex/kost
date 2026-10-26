//! Tests for path ignoring via `lint.ignore` in YAML configurations.

use lint::config::ConfigLoader;
use lint::config::error::ConfigError;

#[test]
fn test_lint_ignore_single_string_compiles_pattern() {
    let yaml = r#"
lint:
  imports:
    - standard
  ignore: "**/generated/**"
"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert_eq!(registry.ignore_patterns.len(), 1);
    assert!(registry.is_path_ignored("src/generated/User.kt"));
    assert!(registry.is_path_ignored("app/sub/generated/Deep.kt"));
    assert!(!registry.is_path_ignored("src/main/User.kt"));
}

#[test]
fn test_lint_ignore_sequence_compiles_multiple_patterns() {
    let yaml = r#"
lint:
  imports:
    - standard
  ignore:
    - "**/build/**"
    - "**/gen/**"
    - "**/*.g.kt"
"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert_eq!(registry.ignore_patterns.len(), 3);
    assert!(registry.is_path_ignored("build/classes/Foo.kt"));
    assert!(registry.is_path_ignored("app/gen/Bar.kt"));
    assert!(registry.is_path_ignored("src/models/Schema.g.kt"));
    assert!(!registry.is_path_ignored("src/models/Schema.kt"));
}

#[test]
fn test_lint_ignore_null_or_omitted_produces_empty_patterns() {
    let yaml_omitted = r#"
lint:
  imports:
    - standard
"#;
    let registry_omitted = ConfigLoader::load_string(yaml_omitted, "kost.yaml").unwrap();
    assert!(registry_omitted.ignore_patterns.is_empty());
    assert!(!registry_omitted.is_path_ignored("src/Main.kt"));

    let yaml_null = r#"
lint:
  imports:
    - standard
  ignore: null
"#;
    let registry_null = ConfigLoader::load_string(yaml_null, "kost.yaml").unwrap();
    assert!(registry_null.ignore_patterns.is_empty());
    assert!(!registry_null.is_path_ignored("src/Main.kt"));
}

#[test]
fn test_lint_ignore_normalizes_windows_backslashes() {
    let yaml = r#"
lint:
  imports:
    - standard
  ignore:
    - "src/generated/**"
"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    // Path using Windows backslashes
    assert!(registry.is_path_ignored(r"src\generated\Foo.kt"));
    assert!(registry.is_path_ignored(r".\src\generated\Foo.kt"));
    assert!(!registry.is_path_ignored(r"src\other\Foo.kt"));
}

#[test]
fn test_lint_ignore_invalid_glob_pattern_fails_with_config_error() {
    let yaml = r#"
lint:
  imports:
    - standard
  ignore:
    - "[z-a]"
"#;
    let err = ConfigLoader::load_string(yaml, "kost.yaml").unwrap_err();
    match err {
        ConfigError::InvalidIgnorePattern { pattern, reason } => {
            assert_eq!(pattern, "[z-a]");
            assert!(!reason.is_empty());
        }
        other => panic!("Expected InvalidIgnorePattern, got: {:?}", other),
    }
}

#[test]
fn test_lint_ignore_empty_or_whitespace_pattern_fails() {
    let yaml_empty = r#"
lint:
  imports:
    - standard
  ignore: ""
"#;
    let err_empty = ConfigLoader::load_string(yaml_empty, "kost.yaml").unwrap_err();
    match err_empty {
        ConfigError::InvalidIgnorePattern { pattern, reason } => {
            assert_eq!(pattern, "");
            assert!(reason.contains("empty"));
        }
        other => panic!(
            "Expected InvalidIgnorePattern for empty string, got: {:?}",
            other
        ),
    }

    let yaml_blank = r#"
lint:
  imports:
    - standard
  ignore:
    - "   "
"#;
    let err_blank = ConfigLoader::load_string(yaml_blank, "kost.yaml").unwrap_err();
    match err_blank {
        ConfigError::InvalidIgnorePattern { pattern, reason } => {
            assert_eq!(pattern, "   ");
            assert!(reason.contains("empty"));
        }
        other => panic!(
            "Expected InvalidIgnorePattern for blank string, got: {:?}",
            other
        ),
    }
}
