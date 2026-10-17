mod ktfmt;

use format::{FileType, FormattingOptions, TrailingCommaStrategy};
use ktfmt::case_config::CaseConfig;

#[test]
fn test_directive_shebang_on_first_line_sets_script_type() {
    let source = "#!/usr/bin/env kscript\nprintln(1)\n";
    let base_options = FormattingOptions::default();
    let config = CaseConfig::parse(source, &base_options).unwrap();

    assert_eq!(config.file_type, FileType::Script);
}

#[test]
fn test_directive_shebang_on_later_line_is_not_script_type() {
    let source = "// some comment\n#!/usr/bin/env kscript\nprintln(1)\n";
    let base_options = FormattingOptions::default();
    let config = CaseConfig::parse(source, &base_options).unwrap();

    assert_eq!(config.file_type, FileType::Regular);
}

#[test]
fn test_directive_file_type_override() {
    let script_source = "// FILE_TYPE SCRIPT\nval x = 1\n";
    let regular_source = "// FILE_TYPE REGULAR\nval x = 1\n";
    let base_options = FormattingOptions::default();

    let script_config = CaseConfig::parse(script_source, &base_options).unwrap();
    let regular_config = CaseConfig::parse(regular_source, &base_options).unwrap();

    assert_eq!(script_config.file_type, FileType::Script);
    assert_eq!(regular_config.file_type, FileType::Regular);
}

#[test]
fn test_directive_max_width_parsed_correctly() {
    let source = "// MAX_WIDTH 50\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let config = CaseConfig::parse(source, &base_options).unwrap();

    assert_eq!(config.options.max_width, 50);
}

#[test]
fn test_directive_block_and_continuation_indent_parsed_correctly() {
    let source = "// BLOCK_INDENT 4\n// CONTINUATION_INDENT 8\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let config = CaseConfig::parse(source, &base_options).unwrap();

    assert_eq!(config.options.block_indent, 4);
    assert_eq!(config.options.continuation_indent, 8);
}

#[test]
fn test_directive_trailing_comma_strategy_parsed_correctly() {
    let base_options = FormattingOptions::default();

    let none_cfg = CaseConfig::parse(
        "// TRAILING_COMMA_STRATEGY NONE\nval x = 1\n",
        &base_options,
    )
    .unwrap();
    let add_cfg = CaseConfig::parse(
        "// TRAILING_COMMA_STRATEGY ONLY_ADD\nval x = 1\n",
        &base_options,
    )
    .unwrap();
    let complete_cfg = CaseConfig::parse(
        "// TRAILING_COMMA_STRATEGY COMPLETE\nval x = 1\n",
        &base_options,
    )
    .unwrap();

    assert_eq!(
        none_cfg.options.trailing_comma_strategy,
        TrailingCommaStrategy::None
    );
    assert_eq!(
        add_cfg.options.trailing_comma_strategy,
        TrailingCommaStrategy::OnlyAdd
    );
    assert_eq!(
        complete_cfg.options.trailing_comma_strategy,
        TrailingCommaStrategy::Complete
    );
}

#[test]
fn test_directive_unused_imports_and_lambda_breaks_flags() {
    let source = "// REMOVE_UNUSED_IMPORTS false\n// PRESERVE_LAMBDA_BREAKS false\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let config = CaseConfig::parse(source, &base_options).unwrap();

    assert!(!config.options.remove_unused_imports);
    assert!(!config.options.preserve_lambda_breaks);
}

#[test]
fn test_directive_check_idempotency_disabled() {
    let source = "// CHECK_IDEMPOTENCY false\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let config = CaseConfig::parse(source, &base_options).unwrap();

    assert!(!config.check_idempotency);
}

#[test]
fn test_directive_misordered_fails_fast() {
    let source = "val x = 1\n// MAX_WIDTH 50\n";
    let base_options = FormattingOptions::default();
    let result = CaseConfig::parse(source, &base_options);

    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("must be listed at top"));
    assert!(err_message.contains("MAX_WIDTH"));
}

#[test]
fn test_directive_unknown_name_fails_fast() {
    let source = "// UNRECOGNIZED_OPTION 42\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let result = CaseConfig::parse(source, &base_options);

    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("Unknown directive 'UNRECOGNIZED_OPTION'"));
}

#[test]
fn test_directive_invalid_integer_fails_fast() {
    let source = "// MAX_WIDTH not_a_number\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let result = CaseConfig::parse(source, &base_options);

    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("Invalid MAX_WIDTH"));
}

#[test]
fn test_directive_print_opts_ignored_cleanly() {
    let source = "// PRINT_OPTS_AFTER_FORMATTING\nval x = 1\n";
    let base_options = FormattingOptions::default();
    let result = CaseConfig::parse(source, &base_options);

    assert!(result.is_ok());
}
