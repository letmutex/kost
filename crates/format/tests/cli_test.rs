use format::cli::run;

#[test]
fn test_cli_run_help_flag_succeeds() {
    let result = run(&["--help".to_string()]);
    assert!(result.is_ok());

    let short_result = run(&["-h".to_string()]);
    assert!(short_result.is_ok());
}

#[test]
fn test_cli_run_unknown_option_returns_error() {
    let result = run(&["--unknown-flag".to_string()]);
    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("unexpected option"));
    assert!(err_message.contains("--unknown-flag"))
}

#[test]
fn test_cli_run_missing_style_argument_returns_error() {
    let result = run(&["--style".to_string()]);
    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("--style"));
    assert!(err_message.contains("requires an argument"));
}

#[test]
fn test_cli_run_invalid_style_value_returns_error() {
    let result = run(&["--style=invalid_style_name".to_string()]);
    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("invalid value 'invalid_style_name'"));
    assert!(err_message.contains("--style"));
}

#[test]
fn test_cli_run_non_existent_target_returns_error() {
    let result = run(&["this_file_does_not_exist_at_all.kt".to_string()]);
    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("Target path not found"));
}

#[test]
fn test_cli_run_cannot_mix_stdin_with_file_targets() {
    let result = run(&["-".to_string(), "Main.kt".to_string()]);
    assert!(result.is_err());
    let err_message = result.unwrap_err();
    assert!(err_message.contains("Cannot mix standard input '-' with file targets"));
}

#[test]
fn test_check_mode_tolerates_parse_errors_by_default() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_check_syntax_ok_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file = temp_dir.join("Invalid.kt");
    std::fs::write(&file, "fun invalid( {\n").unwrap();

    let result = run(&["--check".to_string(), file.to_string_lossy().to_string()]);
    assert!(result.is_ok());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_check_mode_fails_on_parse_errors_with_reject_flag() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_check_syntax_rej_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file = temp_dir.join("Invalid.kt");
    std::fs::write(&file, "fun invalid( {\n").unwrap();

    let result = run(&[
        "--check".to_string(),
        "--reject-parse-errors".to_string(),
        file.to_string_lossy().to_string(),
    ]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Formatting check failed with 1 error(s)"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_check_mode_unformatted_file_fails_regardless_of_parse_errors() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_check_mixed_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let invalid_file = temp_dir.join("Invalid.kt");
    std::fs::write(&invalid_file, "fun invalid( {\n").unwrap();
    let unformatted_file = temp_dir.join("Unformatted.kt");
    std::fs::write(&unformatted_file, "fun foo() {\n  println(1)\n}\n").unwrap();

    let result = run(&[
        "--check".to_string(),
        invalid_file.to_string_lossy().to_string(),
        unformatted_file.to_string_lossy().to_string(),
    ]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("1 file(s) need formatting"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_check_file_classifies_parse_error_outcome() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_check_outcome_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let file = temp_dir.join("SyntaxErr.kt");
    std::fs::write(&file, "class Broken {\n").unwrap();

    let outcome = format::check::check_file(format::StylePreset::Kotlinlang, None, &file, false);
    match outcome {
        format::check::CheckOutcome::ParseError {
            position, message, ..
        } => {
            assert!(position.line >= 1);
            assert!(!message.is_empty());
        }
        _ => panic!("Expected ParseError outcome"),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_check_mode_unformatted_and_parse_error_combined_failure_with_reject_flag() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_check_comb_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let invalid_file = temp_dir.join("Invalid.kt");
    std::fs::write(&invalid_file, "fun invalid( {\n").unwrap();
    let unformatted_file = temp_dir.join("Unformatted.kt");
    std::fs::write(&unformatted_file, "fun foo() {\n  println(1)\n}\n").unwrap();

    let result = run(&[
        "--check".to_string(),
        "--reject-parse-errors".to_string(),
        invalid_file.to_string_lossy().to_string(),
        unformatted_file.to_string_lossy().to_string(),
    ]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("1 file(s) need formatting and 1 error(s) encountered"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_editorconfig_ij_formatter_disabled_skips_in_place_modifications() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_fmt_skip_inplace_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ec_path = temp_dir.join(".editorconfig");
    std::fs::write(
        &ec_path,
        "root = true\n\n[*.kt]\nij_formatter_enabled = false\n",
    )
    .unwrap();

    let unformatted_file = temp_dir.join("Target.kt");
    let original_content = "fun foo() {\n  println(1)\n}\n";
    std::fs::write(&unformatted_file, original_content).unwrap();

    let result = run(&[unformatted_file.to_string_lossy().to_string()]);
    assert!(result.is_ok());

    let content_after = std::fs::read_to_string(&unformatted_file).unwrap();
    assert_eq!(
        content_after, original_content,
        "File with ij_formatter_enabled = false must not be modified"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_editorconfig_ktfmt_disabled_bypasses_check_mode_failure() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_fmt_skip_check_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ec_path = temp_dir.join(".editorconfig");
    std::fs::write(&ec_path, "root = true\n\n[*.kt]\nktfmt = disabled\n").unwrap();

    let unformatted_file = temp_dir.join("Target.kt");
    std::fs::write(&unformatted_file, "fun foo() {\n  println(1)\n}\n").unwrap();

    let result = run(&[
        "--check".to_string(),
        unformatted_file.to_string_lossy().to_string(),
    ]);
    assert!(
        result.is_ok(),
        "Check mode must succeed when ktfmt = disabled"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_editorconfig_child_section_re_enables_formatting_for_subpath() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_fmt_re_enable_", &pid));
    let sub_dir = temp_dir.join("formatted");
    std::fs::create_dir_all(&sub_dir).unwrap();

    let ec_path = temp_dir.join(".editorconfig");
    std::fs::write(
        &ec_path,
        "root = true\n\n[*.kt]\nij_formatter_enabled = false\n\n[formatted/**.kt]\nij_formatter_enabled = true\n",
    )
    .unwrap();

    let skipped_file = temp_dir.join("Skipped.kt");
    let skipped_content = "fun skip() {\n  println(1)\n}\n";
    std::fs::write(&skipped_file, skipped_content).unwrap();

    let formatted_file = sub_dir.join("Formatted.kt");
    let unformatted_content = "fun fmt() {\n  println(1)\n}\n";
    std::fs::write(&formatted_file, unformatted_content).unwrap();

    let result = run(&[
        skipped_file.to_string_lossy().to_string(),
        formatted_file.to_string_lossy().to_string(),
    ]);
    assert!(result.is_ok());

    let skipped_after = std::fs::read_to_string(&skipped_file).unwrap();
    assert_eq!(
        skipped_after, skipped_content,
        "Parent section should keep formatting disabled"
    );

    let formatted_after = std::fs::read_to_string(&formatted_file).unwrap();
    assert_ne!(
        formatted_after, unformatted_content,
        "Child section re-enabling formatting must format the file"
    );
    assert!(formatted_after.contains("    println(1)"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_no_editorconfig_flag_ignores_formatter_disabled_rules() {
    let pid = std::process::id().to_string();
    let temp_dir = std::env::temp_dir().join(common::concat_str!("kost_fmt_no_ec_", &pid));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let ec_path = temp_dir.join(".editorconfig");
    std::fs::write(
        &ec_path,
        "root = true\n\n[*.kt]\nij_formatter_enabled = false\n",
    )
    .unwrap();

    let file = temp_dir.join("Target.kt");
    std::fs::write(&file, "fun foo() {\n  println(1)\n}\n").unwrap();

    // Check with --no-editorconfig: should detect unformatted file despite .editorconfig
    let check_result = run(&[
        "--check".to_string(),
        "--no-editorconfig".to_string(),
        file.to_string_lossy().to_string(),
    ]);
    assert!(
        check_result.is_err(),
        "--no-editorconfig must bypass disabled rules and report unformatted file"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
