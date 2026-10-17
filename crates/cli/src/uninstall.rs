//! CLI argument parsing and options for `kost uninstall`.

use crate::error::CliError;

/// Parsed command line arguments for the `uninstall` command.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UninstallArgs {
    /// Whether to proceed without an interactive confirmation prompt.
    pub yes: bool,
    /// Whether help was requested for the uninstall subcommand.
    pub help: bool,
}

impl UninstallArgs {
    /// Parses arguments for the uninstall command.
    pub fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut yes = false;
        let mut help = false;

        for argument in arguments {
            if argument == "-h" || argument == "--help" {
                help = true;
            } else if argument == "-y" || argument == "--yes" {
                yes = true;
            } else if argument.starts_with('-') && argument != "-" {
                return Err(CliError::UnknownOption(argument.to_string()));
            } else {
                return Err(CliError::UnknownCommand(argument.to_string()));
            }
        }

        Ok(Self { yes, help })
    }
}

/// Renders help text for the `kost uninstall` command, auto-detecting terminal coloring.
pub fn render_uninstall_help() -> String {
    render_uninstall_help_styled(common::should_colorize_stdout())
}

/// Renders help text for the `kost uninstall` command with optional ANSI color styling.
pub fn render_uninstall_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str("kost-uninstall - Uninstall the kost CLI binary\n\n");

    crate::style::push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost uninstall ");
    crate::style::push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push_str("\n\n");

    crate::style::push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    crate::style::push_option(&mut buffer, "-y, --yes", colorize);
    buffer.push_str("              Confirm uninstallation without prompting\n    ");
    crate::style::push_option(&mut buffer, "-h, --help", colorize);
    buffer.push_str("             Print help information\n");

    buffer
}
