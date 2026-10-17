use cli::{CliCommand, CliError, parse_args};
use common::decide_color;

#[test]
fn test_default_empty_arguments_yields_help() {
    let empty_args: Vec<String> = Vec::new();
    let result = parse_args(&empty_args);
    assert_eq!(result, Ok(CliCommand::Help));
}

#[test]
fn test_help_flag_variants() {
    assert_eq!(parse_args(&[String::from("-h")]), Ok(CliCommand::Help));
    assert_eq!(parse_args(&[String::from("--help")]), Ok(CliCommand::Help));
}

#[test]
fn test_version_flag_variants() {
    assert_eq!(parse_args(&[String::from("-v")]), Ok(CliCommand::Version));
    assert_eq!(
        parse_args(&[String::from("--version")]),
        Ok(CliCommand::Version)
    );
}

#[test]
fn test_language_server_flag_variants() {
    assert_eq!(
        parse_args(&[String::from("-ls")]),
        Ok(CliCommand::LanguageServer)
    );
    assert_eq!(
        parse_args(&[String::from("--language-server")]),
        Ok(CliCommand::LanguageServer)
    );
}

#[test]
fn test_subcommand_parsing() {
    assert_eq!(
        parse_args(&[String::from("update"), String::from("--check")]),
        Ok(CliCommand::Update(vec![String::from("--check")]))
    );
    assert_eq!(
        parse_args(&[String::from("lint"), String::from("src/")]),
        Ok(CliCommand::Lint(vec![String::from("src/")]))
    );
    assert_eq!(
        parse_args(&[String::from("fmt"), String::from("--check")]),
        Ok(CliCommand::Format(vec![String::from("--check")]))
    );
    assert_eq!(
        parse_args(&[String::from("format")]),
        Ok(CliCommand::Format(Vec::new()))
    );
}

#[test]
fn test_unknown_option_error() {
    let result = parse_args(&[String::from("--invalid")]);
    assert_eq!(
        result,
        Err(CliError::UnknownOption(String::from("--invalid")))
    );
}

#[test]
fn test_unknown_command_error() {
    let result = parse_args(&[String::from("unknown")]);
    assert_eq!(
        result,
        Err(CliError::UnknownCommand(String::from("unknown")))
    );
}

#[test]
fn test_color_decision_rules() {
    // Interactive terminal with no special environment variables enables colors
    assert!(decide_color(true, None, None));

    // Non-interactive stream disables colors
    assert!(!decide_color(false, None, None));

    // Empty NO_COLOR string does not disable colors according to NO_COLOR specification
    assert!(decide_color(true, Some(std::ffi::OsStr::new("")), None));

    // Non-empty NO_COLOR disables colors
    assert!(!decide_color(true, Some(std::ffi::OsStr::new("1")), None));
    assert!(!decide_color(
        true,
        Some(std::ffi::OsStr::new("true")),
        None
    ));

    // TERM=dumb disables colors
    assert!(!decide_color(true, None, Some("dumb")));

    // Standard TERM enables colors on interactive terminal
    assert!(decide_color(true, None, Some("xterm-256color")));
}
