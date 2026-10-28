//! CLI argument parsing and options for `kost fmt`.

use crate::error::CliError;
use crate::print_mode::PrintMode;

/// Style preset selection for the `fmt` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatStyle {
    /// Kotlinlang official style guide (4-space indent, complete trailing comma)
    #[default]
    Kotlinlang,
    /// Meta style (2-space indent, 4 continuation)
    Meta,
    /// Google style (2-space block and continuation)
    Google,
}

impl FormatStyle {
    fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier.to_ascii_lowercase().as_str() {
            "kotlinlang" => Some(Self::Kotlinlang),
            "meta" => Some(Self::Meta),
            "google" => Some(Self::Google),
            _ => None,
        }
    }
}

/// Parsed command line arguments for the `fmt` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatArgs {
    /// Target file paths, directories, or glob patterns to format.
    pub targets: Vec<String>,
    /// Whether to check formatting and output diff without modifying files.
    pub check: bool,
    /// Active style preset (defaults to Kotlinlang).
    pub style: FormatStyle,
    /// Diff presentation mode in check mode (defaults to Auto).
    pub print_mode: PrintMode,
    /// Whether to discover and respect .editorconfig files (defaults to true).
    pub editorconfig_enabled: bool,
    /// Path to explicit or fallback .editorconfig file.
    pub editorconfig_path: Option<String>,
    /// Virtual file path for stdin inputs.
    pub stdin_path: Option<String>,
    /// Whether to fail check mode on Tree-sitter parse errors (defaults to false).
    pub reject_parse_errors: bool,
    /// Whether help was requested for the fmt subcommand.
    pub help: bool,
}

impl Default for FormatArgs {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            check: false,
            style: FormatStyle::default(),
            print_mode: PrintMode::default(),
            editorconfig_enabled: true,
            editorconfig_path: None,
            stdin_path: None,
            reject_parse_errors: false,
            help: false,
        }
    }
}

impl FormatArgs {
    /// Parses arguments for the format command.
    pub fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut targets = Vec::new();
        let mut check = false;
        let mut style = FormatStyle::default();
        let mut print_mode = PrintMode::default();
        let mut editorconfig_enabled = true;
        let mut editorconfig_path = None;
        let mut stdin_path = None;
        let mut reject_parse_errors = false;
        let mut help = false;
        let mut argument_iterator = arguments.iter();

        while let Some(argument) = argument_iterator.next() {
            if argument == "--" {
                targets.extend(argument_iterator.cloned());
                break;
            } else if argument == "-h" || argument == "--help" {
                help = true;
            } else if argument == "-c" || argument == "--check" {
                check = true;
            } else if argument == "--reject-parse-errors" {
                reject_parse_errors = true;
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
            } else if argument == "--kotlinlang-style" {
                style = FormatStyle::Kotlinlang;
            } else if argument == "--meta-style" {
                style = FormatStyle::Meta;
            } else if argument == "--google-style" {
                style = FormatStyle::Google;
            } else if let Some(val) =
                crate::parse_value_option(argument, "--style", &mut argument_iterator)?
            {
                style = parse_style_value("--style", &val)?;
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
            targets,
            check,
            style,
            print_mode,
            editorconfig_enabled,
            editorconfig_path,
            stdin_path,
            reject_parse_errors,
            help,
        })
    }
}

fn parse_style_value(option_name: &str, raw_value: &str) -> Result<FormatStyle, CliError> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Err(CliError::MissingOptionValue(option_name.to_string()));
    }
    FormatStyle::from_identifier(trimmed).ok_or_else(|| CliError::InvalidOptionValue {
        option: option_name.to_string(),
        value: raw_value.to_string(),
    })
}

/// Renders help text for the `kost fmt` command, auto-detecting terminal coloring.
pub fn render_format_help() -> String {
    render_format_help_styled(common::should_colorize_stdout())
}

/// Renders help text for the `kost fmt` command with optional ANSI color styling.
pub fn render_format_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("Format Kotlin source files\n\n");

    crate::style::push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost fmt ");
    crate::style::push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "[PATHS/GLOBS]...", colorize);
    buffer.push_str("\n    cat File.kt | kost fmt ");
    crate::style::push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push_str(" -\n\n");

    crate::style::push_header(&mut buffer, "ARGS:", colorize);
    buffer.push_str("\n    ");
    crate::style::push_placeholder(&mut buffer, "<PATHS/GLOBS>...", colorize);
    buffer.push_str("        Kotlin files, directories, or glob patterns to format [default: auto-discover .kt/.kts]\n\n");

    crate::style::push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    crate::style::push_option(&mut buffer, "-c, --check", colorize);
    buffer
        .push_str("             Check formatting and display diffs without modifying files\n    ");
    crate::style::push_option(&mut buffer, "--style", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<PRESET>", colorize);
    buffer.push_str("        Style preset: 'kotlinlang' (default), 'meta', or 'google'\n    ");
    crate::style::push_option(&mut buffer, "--kotlinlang-style", colorize);
    buffer.push_str("      Shortcut for '--style kotlinlang' (default: 4-space indent)\n    ");
    crate::style::push_option(&mut buffer, "--meta-style", colorize);
    buffer.push_str("            Shortcut for '--style meta'\n    ");
    crate::style::push_option(&mut buffer, "--google-style", colorize);
    buffer.push_str("          Shortcut for '--style google'\n    ");
    crate::style::push_option(&mut buffer, "--editorconfig", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<PATH>", colorize);
    buffer.push_str("    Explicit or fallback .editorconfig file path\n    ");
    crate::style::push_option(&mut buffer, "--no-editorconfig", colorize);
    buffer.push_str("       Disable .editorconfig discovery and property inheritance\n    ");
    crate::style::push_option(&mut buffer, "--print-mode", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<MODE>", colorize);
    buffer.push_str(
        "     Presentation mode: 'auto' (default), 'full', 'truncated', or 'collapsed'\n    ",
    );
    crate::style::push_option(&mut buffer, "--stdin-path", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<PATH>", colorize);
    buffer.push_str("      Virtual file path for stdin formatting\n    ");
    crate::style::push_option(&mut buffer, "--reject-parse-errors", colorize);
    buffer.push_str("   Treat syntax/parse errors as failures in check mode\n    ");
    crate::style::push_option(&mut buffer, "-h, --help", colorize);
    buffer.push_str("              Print help information\n");

    buffer
}
