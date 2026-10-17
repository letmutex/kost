//! CLI argument parsing and options for `kost update`.

use crate::error::CliError;

/// Parsed command line arguments for the `update` command.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateArgs {
    /// Whether to check if an update is available without installing it.
    pub check: bool,
    /// Target release version or tag (e.g. "v0.1.0" or "0.1.0").
    pub version: Option<String>,
    /// Whether to force installation even if the version matches the target version.
    pub force: bool,
    /// Whether help was requested for the update subcommand.
    pub help: bool,
}

impl UpdateArgs {
    /// Parses arguments for the update command.
    pub fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut check = false;
        let mut version = None;
        let mut force = false;
        let mut help = false;
        let mut argument_iterator = arguments.iter();

        while let Some(argument) = argument_iterator.next() {
            if argument == "-h" || argument == "--help" {
                help = true;
            } else if argument == "-c" || argument == "--check" {
                check = true;
            } else if argument == "-f" || argument == "--force" {
                force = true;
            } else if let Some(val) =
                crate::parse_value_option(argument, "--version", &mut argument_iterator)?
            {
                version = Some(val);
            } else if let Some(val) =
                crate::parse_value_option(argument, "-v", &mut argument_iterator)?
            {
                version = Some(val);
            } else if argument.starts_with('-') && argument != "-" {
                return Err(CliError::UnknownOption(argument.to_string()));
            } else {
                return Err(CliError::UnknownCommand(argument.to_string()));
            }
        }

        Ok(Self {
            check,
            version,
            force,
            help,
        })
    }
}

/// Renders help text for the `kost update` command, auto-detecting terminal coloring.
pub fn render_update_help() -> String {
    render_update_help_styled(common::should_colorize_stdout())
}

/// Renders help text for the `kost update` command with optional ANSI color styling.
pub fn render_update_help_styled(colorize: bool) -> String {
    let mut buffer = String::new();
    buffer.push_str(
        "kost-update - Update the kost CLI binary to the latest or specified release\n\n",
    );

    crate::style::push_header(&mut buffer, "USAGE:", colorize);
    buffer.push_str("\n    kost update ");
    crate::style::push_placeholder(&mut buffer, "[OPTIONS]", colorize);
    buffer.push_str("\n\n");

    crate::style::push_header(&mut buffer, "OPTIONS:", colorize);
    buffer.push_str("\n    ");
    crate::style::push_option(&mut buffer, "-c, --check", colorize);
    buffer.push_str("              Check if a newer version is available without updating\n    ");
    crate::style::push_option(&mut buffer, "-v, --version", colorize);
    buffer.push(' ');
    crate::style::push_placeholder(&mut buffer, "<TAG>", colorize);
    buffer.push_str("     Install a specific release version [e.g. 'v0.1.0']\n    ");
    crate::style::push_option(&mut buffer, "-f, --force", colorize);
    buffer.push_str("              Force installation even if already up to date\n    ");
    crate::style::push_option(&mut buffer, "-h, --help", colorize);
    buffer.push_str("               Print help information\n");

    buffer
}
