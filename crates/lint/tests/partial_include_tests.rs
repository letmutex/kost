//! Tests for partial includes, inline qualified rule imports, and structured selection lists.

use lint::config::ConfigLoader;

#[test]
fn test_inline_partial_include_from_standard_ast() {
    let yaml = r#"
lint:
  includes:
    - standard:no-semi

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert!(registry.ast_dispatch.contains_key(";"));
    assert!(!registry.ast_dispatch.contains_key(":"));
    assert!(registry.stat_dispatch.is_empty());
}

#[test]
fn test_inline_partial_include_from_statistics() {
    let yaml = r#"
lint:
  includes:
    - statistics:long-function

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert!(registry.ast_dispatch.is_empty());
    let function_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    assert_eq!(function_rules.len(), 1);
    assert_eq!(function_rules[0].id, "statistics:long-function");
}

#[test]
fn test_structured_partial_include_from_standard_and_statistics() {
    let yaml = r#"
lint:
  includes:
    - path: standard
      rules:
        - no-semi
    - path: statistics
      rules:
        - long-function

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert!(registry.ast_dispatch.contains_key(";"));
    assert!(!registry.ast_dispatch.contains_key(":"));
    let function_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    assert_eq!(function_rules.len(), 1);
    assert_eq!(function_rules[0].id, "statistics:long-function");
}

#[test]
fn test_partial_include_from_external_file() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_test_partial_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let shared_yaml = temp_dir.join("shared.yaml");
    std::fs::write(
        &shared_yaml,
        r#"
lint:
  rules:
    - id: custom-one
      severity: warning
      where:
        function.lines: gt(50)
      message: "Function too long"
    - id: custom-two
      severity: error
      where:
        class.lines: gt(200)
      message: "Class too long"

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
        - custom-one

"#,
    )
    .unwrap();

    let registry = ConfigLoader::load_file(&main_yaml).unwrap();
    let func_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    assert_eq!(func_rules.len(), 1);
    assert_eq!(func_rules[0].id, "shared.yaml:custom-one");
    assert!(
        !registry
            .stat_dispatch
            .contains_key(&lint::types::Scope::Class)
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_inline_partial_include_from_external_file() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_test_inline_file_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let shared_yaml = temp_dir.join("shared.yaml");
    std::fs::write(
        &shared_yaml,
        r#"
lint:
  rules:
    - id: custom-rule
      severity: warning
      where:
        function.lines: gt(50)
      message: "Function too long"

"#,
    )
    .unwrap();

    let main_yaml = temp_dir.join("main.yaml");
    std::fs::write(
        &main_yaml,
        r#"
lint:
  includes:
    - ./shared.yaml:custom-rule

"#,
    )
    .unwrap();

    let registry = ConfigLoader::load_file(&main_yaml).unwrap();
    let func_rules = registry
        .stat_dispatch
        .get(&lint::types::Scope::Function)
        .unwrap();
    assert_eq!(func_rules[0].id, "shared.yaml:custom-rule");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_excluding_rule_via_severity_none_override() {
    let yaml = r#"
lint:
  includes:
    - path: standard
      rules:
        - no-semi
        - colon-spacing
  rules:
    - id: standard:colon-spacing
      severity: none

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert!(registry.ast_dispatch.contains_key(";"));
    assert!(!registry.ast_dispatch.contains_key(":"));
}

#[test]
fn test_standard_ast_rule_constructibility_bijection() {
    let ids = lint::rules::standard::standard_ast_rule_ids();
    assert!(!ids.is_empty());
    for id in ids {
        let rule = lint::rules::standard::create_standard_ast_rule(id)
            .expect("Every standard AST rule ID must be constructible");
        assert_eq!(rule.id(), *id);
    }
}

#[test]
fn test_excluding_stat_rule_via_severity_none_override() {
    let yaml = r#"
lint:
  includes:
    - path: statistics
      rules:
        - long-function
  rules:
    - id: statistics:long-function
      severity: none

"#;
    let registry = ConfigLoader::load_string(yaml, "kost.yaml").unwrap();
    assert!(
        !registry
            .stat_dispatch
            .contains_key(&lint::types::Scope::Function)
    );
}
