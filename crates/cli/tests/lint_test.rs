use cli::{CliError, LintArgs, PrintMode};

#[test]
fn test_parse_lint_print_mode_default_auto() {
    let args = LintArgs::parse(&[]).unwrap();
    assert_eq!(args.print_mode, PrintMode::Auto);
}

#[test]
fn test_parse_lint_print_mode_space_separated() {
    let raw = vec![
        "--print-mode".to_string(),
        "full".to_string(),
        "src/".to_string(),
    ];
    let args = LintArgs::parse(&raw).unwrap();
    assert_eq!(args.print_mode, PrintMode::Full);
    assert_eq!(args.targets, vec!["src/"]);
}

#[test]
fn test_parse_lint_print_mode_equals_syntax() {
    let raw = vec!["--print-mode=truncated".to_string()];
    let args = LintArgs::parse(&raw).unwrap();
    assert_eq!(args.print_mode, PrintMode::Truncated);
}

#[test]
fn test_parse_lint_print_mode_collapsed() {
    let raw = vec!["--print-mode".to_string(), "collapsed".to_string()];
    let args = LintArgs::parse(&raw).unwrap();
    assert_eq!(args.print_mode, PrintMode::Collapsed);
}

#[test]
fn test_parse_lint_print_mode_invalid_value() {
    let raw = vec!["--print-mode=invalid_mode".to_string()];
    let err = LintArgs::parse(&raw).unwrap_err();
    assert_eq!(
        err,
        CliError::InvalidOptionValue {
            option: "--print-mode".to_string(),
            value: "invalid_mode".to_string(),
        }
    );
}

#[test]
fn test_parse_lint_print_mode_missing_value() {
    let raw = vec!["--print-mode".to_string()];
    let err = LintArgs::parse(&raw).unwrap_err();
    assert_eq!(
        err,
        CliError::MissingOptionValue("--print-mode".to_string())
    );
}

#[test]
fn test_parse_lint_deny_warnings_variants() {
    let variants = [
        vec!["--deny".to_string(), "warnings".to_string()],
        vec!["--deny=warnings".to_string()],
        vec!["-D".to_string(), "warnings".to_string()],
        vec!["-D=warnings".to_string()],
        vec!["-Dwarnings".to_string()],
    ];

    for raw in variants {
        let args = LintArgs::parse(&raw).unwrap();
        assert!(args.deny_warnings);
    }
}

#[test]
fn test_parse_lint_deny_invalid_value() {
    let raw = vec!["--deny".to_string(), "invalid".to_string()];
    let err = LintArgs::parse(&raw).unwrap_err();
    assert_eq!(
        err,
        CliError::InvalidOptionValue {
            option: "--deny".to_string(),
            value: "invalid".to_string(),
        }
    );

    let raw_short = vec!["-D".to_string(), "invalid".to_string()];
    let err_short = LintArgs::parse(&raw_short).unwrap_err();
    assert_eq!(
        err_short,
        CliError::InvalidOptionValue {
            option: "-D".to_string(),
            value: "invalid".to_string(),
        }
    );
}

#[test]
fn test_parse_lint_deny_missing_value() {
    let raw = vec!["--deny".to_string()];
    let err = LintArgs::parse(&raw).unwrap_err();
    assert_eq!(err, CliError::MissingOptionValue("--deny".to_string()));

    let raw_short = vec!["-D".to_string()];
    let err_short = LintArgs::parse(&raw_short).unwrap_err();
    assert_eq!(err_short, CliError::MissingOptionValue("-D".to_string()));
}
