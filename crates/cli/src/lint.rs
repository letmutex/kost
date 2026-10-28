//! CLI argument parsing and options for `kost lint`.

use crate::error::CliError;
use crate::print_mode::PrintMode;

/// Parsed command line arguments for the `lint` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintArgs {
    /// Comma-separated list of rulesets (e.g. "standard", "statistics").
    pub rulesets: Option<String>,
    /// File paths, directories, or glob patterns to lint.
    pub targets: Vec<String>,
    /// Presentation mode for diagnostics (defaults to Auto).
    pub print_mode: PrintMode,
    /// Whether to discover and respect .editorconfig files (defaults to true).
    pub editorconfig_enabled: bool,
    /// Path to explicit or fallback .editorconfig file.
    pub editorconfig_path: Option<String>,
    /// Virtual file path for stdin inputs.
    pub stdin_path: Option<String>,
    /// Whether to treat warnings as errors.
    pub deny_warnings: bool,
    /// Whether help was requested for the lint subcommand.
    pub help: bool,
}

impl Default for LintArgs {
    fn default() -> Self {
        Self {
            rulesets: None,
            targets: Vec::new(),
            print_mode: PrintMode::default(),
            editorconfig_enabled: true,
            editorconfig_path: None,
            stdin_path: None,
            deny_warnings: false,
            help: false,
        }
    }
}

impl LintArgs {
    /// Parses arguments for the lint command.
    pub fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut rulesets = None;
        let mut targets = Vec::new();
        let mut print_mode = PrintMode::default();
        let mut editorconfig_enabled = true;
        let mut editorconfig_path = None;
        let mut stdin_path = None;
        let mut deny_warnings = false;
        let mut help = false;
        let mut argument_iterator = arguments.iter();
        while let Some(argument) = argument_iterator.next() {
            if argument == "--" {
                targets.extend(argument_iterator.cloned());
                break;
            } else if argument == "-h" || argument == "--help" {
                help = true;
            } else if argument == "--no-editorconfig" {
                editorconfig_enabled = false;
            } else if let Some(val) =
                crate::parse_value_option(argument, "--editorconfig", &mut argument_iterator)?
            {
                editorconfig_path = Some(val);
            } else if let Some(val) =
                crate::parse_value_option(argument, "--stdin-path", &mut argument_iterator)?
            {
                stdin_path = Some(val);
            } else if let Some(val) =
                crate::parse_value_option(argument, "--deny", &mut argument_iterator)?
            {
                deny_warnings = parse_deny_value("--deny", &val)?;
            } else if let Some(val) =
                crate::parse_value_option(argument, "-D", &mut argument_iterator)?
            {
                deny_warnings = parse_deny_value("-D", &val)?;
            } else if let Some(val) =
                crate::parse_value_option(argument, "--rulesets", &mut argument_iterator)?
            {
                rulesets = Some(val);
            } else if let Some(val) =
                crate::parse_value_option(argument, "--print-mode", &mut argument_iterator)?
            {
                print_mode = PrintMode::parse_option("--print-mode", &val)?;
            } else if argument.starts_with('-') && argument != "-" {
                return Err(CliError::UnknownOption(argument.to_string()));
            } else {
                targets.push(argument.to_string());
            }
        }

        Ok(Self {
            rulesets,
            targets,
            print_mode,
            editorconfig_enabled,
            editorconfig_path,
            stdin_path,
            deny_warnings,
            help,
        })
    }
}

fn parse_deny_value(option_name: &str, raw_value: &str) -> Result<bool, CliError> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Err(CliError::MissingOptionValue(option_name.to_string()));
    }
    match trimmed {
        "warnings" => Ok(true),
        _ => Err(CliError::InvalidOptionValue {
            option: option_name.to_string(),
            value: raw_value.to_string(),
        }),
    }
}

/// Renders help text for the `kost lint` command, auto-detecting terminal coloring.
pub fn render_lint_help() -> String {
    render_lint_help_styled(common::should_colorize_stdout())
}

/// Renders help text for the `kost lint` command with optional ANSI color styling.
pub fn render_lint_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("Lint Kotlin source files\n\n");

    crate::style::push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost lint ");
    crate::style::push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "[PATHS/GLOBS]...", colorize);
    buffer.push_str("\n\n");

    crate::style::push_header(&mut buffer, "ARGS:", colorize);
    buffer.push_str("\n    ");
    crate::style::push_placeholder(&mut buffer, "<PATHS/GLOBS>...", colorize);
    buffer.push_str("    Kotlin files, directories, or glob patterns to lint [default: auto-discover .kt/.kts]\n\n");

    crate::style::push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    crate::style::push_option(&mut buffer, "--rulesets", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<LIST>", colorize);
    buffer.push_str("   Comma-separated list of rulesets (defaults to standard or kost.yml/kost.yaml if present)\n    ");
    crate::style::push_option(&mut buffer, "--editorconfig", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<PATH>", colorize);
    buffer.push_str("    Explicit or fallback .editorconfig file path\n    ");
    crate::style::push_option(&mut buffer, "--no-editorconfig", colorize);
    buffer.push_str("       Disable .editorconfig discovery and property inheritance\n    ");
    crate::style::push_option(&mut buffer, "-D, --deny", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<WARNINGS>", colorize);
    buffer.push_str("  Treat warnings as errors [e.g. 'warnings']\n    ");
    crate::style::push_option(&mut buffer, "--stdin-path", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<PATH>", colorize);
    buffer.push_str("      Virtual file path for stdin linting\n    ");
    crate::style::push_option(&mut buffer, "--print-mode", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<MODE>", colorize);
    buffer
        .push_str("    Presentation mode: auto, full, truncated, collapsed [default: auto]\n    ");
    crate::style::push_option(&mut buffer, "-h, --help", colorize);
    buffer.push_str("          Print help information\n");

    buffer
}

#[cfg(test)]
mod tests {
    use super::LintArgs;
    use crate::CliError;

    #[test]
    fn test_parse_lint_empty_args() {
        let args = LintArgs::parse(&[]).unwrap();
        assert_eq!(args.rulesets, None);
        assert!(args.targets.is_empty());
        assert!(!args.help);
    }

    #[test]
    fn test_parse_lint_rulesets_flag() {
        let raw = vec![
            "--rulesets".to_string(),
            "standard,statistics".to_string(),
            "src/Main.kt".to_string(),
        ];
        let args = LintArgs::parse(&raw).unwrap();
        assert_eq!(args.rulesets.as_deref(), Some("standard,statistics"));
        assert_eq!(args.targets, vec!["src/Main.kt"]);
        assert!(!args.help);
    }

    #[test]
    fn test_parse_lint_rulesets_equals_syntax() {
        let raw = vec!["--rulesets=standard".to_string(), "src/".to_string()];
        let args = LintArgs::parse(&raw).unwrap();
        assert_eq!(args.rulesets.as_deref(), Some("standard"));
        assert_eq!(args.targets, vec!["src/"]);
    }

    #[test]
    fn test_parse_lint_missing_rulesets_value() {
        let raw = vec!["--rulesets".to_string()];
        let err = LintArgs::parse(&raw).unwrap_err();
        assert_eq!(err, CliError::MissingOptionValue("--rulesets".to_string()));
    }

    #[test]
    fn test_parse_lint_empty_rulesets_equals_value() {
        let raw = vec!["--rulesets=".to_string()];
        let err = LintArgs::parse(&raw).unwrap_err();
        assert_eq!(err, CliError::MissingOptionValue("--rulesets".to_string()));
    }

    #[test]
    fn test_parse_lint_double_dash_delimiter() {
        let raw = vec![
            "--rulesets".to_string(),
            "standard".to_string(),
            "--".to_string(),
            "-special-file.kt".to_string(),
            "src/Main.kt".to_string(),
        ];
        let args = LintArgs::parse(&raw).unwrap();
        assert_eq!(args.rulesets.as_deref(), Some("standard"));
        assert_eq!(args.targets, vec!["-special-file.kt", "src/Main.kt"]);
    }

    #[test]
    fn test_parse_lint_unknown_option() {
        let raw = vec!["--invalid".to_string()];
        let err = LintArgs::parse(&raw).unwrap_err();
        assert_eq!(err, CliError::UnknownOption("--invalid".to_string()));
    }

    #[test]
    fn test_parse_lint_unknown_short_option() {
        let raw = vec!["-x".to_string()];
        let err = LintArgs::parse(&raw).unwrap_err();
        assert_eq!(err, CliError::UnknownOption("-x".to_string()));
    }

    #[test]
    fn test_parse_lint_rulesets_followed_by_flag_fails() {
        let raw = vec!["--rulesets".to_string(), "--help".to_string()];
        let err = LintArgs::parse(&raw).unwrap_err();
        assert_eq!(err, CliError::MissingOptionValue("--rulesets".to_string()));
    }

    #[test]
    fn test_parse_lint_rulesets_followed_by_double_dash_fails() {
        let raw = vec![
            "--rulesets".to_string(),
            "--".to_string(),
            "src/Main.kt".to_string(),
        ];
        let err = LintArgs::parse(&raw).unwrap_err();
        assert_eq!(err, CliError::MissingOptionValue("--rulesets".to_string()));
    }

    #[test]
    fn test_parse_lint_help() {
        let raw = vec!["--help".to_string()];
        let args = LintArgs::parse(&raw).unwrap();
        assert!(args.help);
    }

    #[test]
    fn test_parse_lint_editorconfig_flags() {
        let raw = vec![
            "--no-editorconfig".to_string(),
            "--stdin-path=src/App.kt".to_string(),
        ];
        let args = LintArgs::parse(&raw).unwrap();
        assert!(!args.editorconfig_enabled);
        assert_eq!(args.stdin_path.as_deref(), Some("src/App.kt"));
    }

    #[test]
    fn test_parse_lint_editorconfig_path_space_separated() {
        let raw = vec![
            "--editorconfig".to_string(),
            ".editorconfig".to_string(),
            "src/Main.kt".to_string(),
        ];
        let args = LintArgs::parse(&raw).unwrap();
        assert!(args.editorconfig_enabled);
        assert_eq!(args.editorconfig_path.as_deref(), Some(".editorconfig"));
        assert_eq!(args.targets, vec!["src/Main.kt".to_string()]);
    }

    #[test]
    fn test_parse_lint_editorconfig_missing_path_value_error() {
        let raw = vec!["--editorconfig".to_string()];
        let result = LintArgs::parse(&raw);
        assert_eq!(
            result.unwrap_err(),
            CliError::MissingOptionValue("--editorconfig".to_string())
        );
    }
}
