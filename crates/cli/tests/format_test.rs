use cli::{CliError, FormatArgs, FormatStyle, PrintMode};

#[test]
fn test_parse_fmt_empty_args_defaults_to_kotlinlang() {
    let args = FormatArgs::parse(&[]).unwrap();
    assert!(!args.check);
    assert_eq!(args.style, FormatStyle::Kotlinlang);
    assert!(args.targets.is_empty());
    assert!(!args.help);
}

#[test]
fn test_parse_fmt_check_flag() {
    let raw = vec!["--check".to_string(), "src/Main.kt".to_string()];
    let args = FormatArgs::parse(&raw).unwrap();
    assert!(args.check);
    assert_eq!(args.targets, vec!["src/Main.kt"]);

    let short_raw = vec!["-c".to_string()];
    let short_args = FormatArgs::parse(&short_raw).unwrap();
    assert!(short_args.check);
}

#[test]
fn test_parse_fmt_style_options() {
    let meta_arg = vec!["--style=meta".to_string()];
    let parsed_meta = FormatArgs::parse(&meta_arg).unwrap();
    assert_eq!(parsed_meta.style, FormatStyle::Meta);

    let google_arg = vec!["--style".to_string(), "google".to_string()];
    let parsed_google = FormatArgs::parse(&google_arg).unwrap();
    assert_eq!(parsed_google.style, FormatStyle::Google);

    let kotlinlang_arg = vec!["--kotlinlang-style".to_string()];
    let parsed_kotlinlang = FormatArgs::parse(&kotlinlang_arg).unwrap();
    assert_eq!(parsed_kotlinlang.style, FormatStyle::Kotlinlang);
}

#[test]
fn test_parse_fmt_stdin_target() {
    let raw = vec!["-".to_string()];
    let args = FormatArgs::parse(&raw).unwrap();
    assert_eq!(args.targets, vec!["-"]);
}

#[test]
fn test_parse_fmt_delimiter_preserves_targets() {
    let raw = vec![
        "--check".to_string(),
        "--".to_string(),
        "-special-name.kt".to_string(),
        "--meta-style".to_string(),
    ];
    let args = FormatArgs::parse(&raw).unwrap();
    assert!(args.check);
    assert_eq!(
        args.targets,
        vec!["-special-name.kt".to_string(), "--meta-style".to_string()]
    );
}

#[test]
fn test_parse_fmt_unknown_option_error() {
    let raw = vec!["--quiet".to_string()];
    let result = FormatArgs::parse(&raw);
    assert_eq!(
        result.unwrap_err(),
        CliError::UnknownOption("--quiet".to_string())
    );
}

#[test]
fn test_parse_fmt_missing_style_value_error() {
    let raw = vec!["--style".to_string()];
    let result = FormatArgs::parse(&raw);
    assert_eq!(
        result.unwrap_err(),
        CliError::MissingOptionValue("--style".to_string())
    );
}

#[test]
fn test_parse_fmt_invalid_style_value_error() {
    let raw = vec!["--style=unknown_preset".to_string()];
    let result = FormatArgs::parse(&raw);
    assert_eq!(
        result.unwrap_err(),
        CliError::InvalidOptionValue {
            option: "--style".to_string(),
            value: "unknown_preset".to_string(),
        }
    );
}

#[test]
fn test_parse_fmt_editorconfig_flags() {
    let raw = vec![
        "--no-editorconfig".to_string(),
        "--stdin-path=src/App.kt".to_string(),
    ];
    let args = FormatArgs::parse(&raw).unwrap();
    assert!(!args.editorconfig_enabled);
    assert_eq!(args.stdin_path.as_deref(), Some("src/App.kt"));
}

#[test]
fn test_parse_fmt_editorconfig_path_space_separated() {
    let raw = vec![
        "--editorconfig".to_string(),
        ".editorconfig".to_string(),
        "src/Main.kt".to_string(),
    ];
    let args = FormatArgs::parse(&raw).unwrap();
    assert!(args.editorconfig_enabled);
    assert_eq!(args.editorconfig_path.as_deref(), Some(".editorconfig"));
    assert_eq!(args.targets, vec!["src/Main.kt".to_string()]);
}

#[test]
fn test_parse_fmt_editorconfig_missing_path_value_error() {
    let raw = vec!["--editorconfig".to_string()];
    let result = FormatArgs::parse(&raw);
    assert_eq!(
        result.unwrap_err(),
        CliError::MissingOptionValue("--editorconfig".to_string())
    );
}

#[test]
fn test_parse_fmt_print_mode_defaults_to_auto() {
    let args = FormatArgs::parse(&[]).unwrap();
    assert_eq!(args.print_mode, PrintMode::Auto);
}

#[test]
fn test_parse_fmt_print_mode_options() {
    let full_arg = vec!["--print-mode=full".to_string()];
    assert_eq!(
        FormatArgs::parse(&full_arg).unwrap().print_mode,
        PrintMode::Full
    );

    let trunc_arg = vec!["--print-mode".to_string(), "truncated".to_string()];
    assert_eq!(
        FormatArgs::parse(&trunc_arg).unwrap().print_mode,
        PrintMode::Truncated
    );

    let collapsed_arg = vec!["--print-mode=collapsed".to_string()];
    assert_eq!(
        FormatArgs::parse(&collapsed_arg).unwrap().print_mode,
        PrintMode::Collapsed
    );

    let auto_arg = vec!["--print-mode".to_string(), "auto".to_string()];
    assert_eq!(
        FormatArgs::parse(&auto_arg).unwrap().print_mode,
        PrintMode::Auto
    );
}

#[test]
fn test_parse_fmt_print_mode_invalid_value_error() {
    let raw = vec!["--print-mode=invalid_mode".to_string()];
    let result = FormatArgs::parse(&raw);
    assert_eq!(
        result.unwrap_err(),
        CliError::InvalidOptionValue {
            option: "--print-mode".to_string(),
            value: "invalid_mode".to_string(),
        }
    );
}

#[test]
fn test_parse_fmt_reject_parse_errors_defaults_to_false() {
    let args = FormatArgs::parse(&[]).unwrap();
    assert!(!args.reject_parse_errors);
}

#[test]
fn test_parse_fmt_reject_parse_errors_flag() {
    let raw = vec!["--check".to_string(), "--reject-parse-errors".to_string()];
    let args = FormatArgs::parse(&raw).unwrap();
    assert!(args.check);
    assert!(args.reject_parse_errors);
}
