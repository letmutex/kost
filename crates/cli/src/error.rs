use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    UnknownOption(String),
    UnknownCommand(String),
    MissingOptionValue(String),
    InvalidOptionValue { option: String, value: String },
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        2
    }

    /// Renders the error message with optional ANSI color styling.
    pub fn render_styled(&self, colorize: bool) -> String {
        let mut buffer = String::with_capacity(96);
        buffer.push_str(crate::style::error_prefix(colorize));
        buffer.push(' ');

        match self {
            Self::UnknownOption(option_name) => {
                buffer.push_str("unexpected option '");
                crate::style::push_option(&mut buffer, option_name, colorize);
                buffer.push('\'');
            }
            Self::UnknownCommand(command_name) => {
                buffer.push_str("unrecognized subcommand '");
                crate::style::push_placeholder(&mut buffer, command_name, colorize);
                buffer.push('\'');
            }
            Self::MissingOptionValue(option_name) => {
                buffer.push_str("option '");
                crate::style::push_option(&mut buffer, option_name, colorize);
                buffer.push_str("' requires an argument");
            }
            Self::InvalidOptionValue { option, value } => {
                buffer.push_str("invalid value '");
                buffer.push_str(value);
                buffer.push_str("' for option '");
                crate::style::push_option(&mut buffer, option, colorize);
                buffer.push('\'');
            }
        }

        buffer.push_str("\n\nFor more information, try '");
        buffer.push_str(crate::style::help_option(colorize));
        buffer.push_str("'.");
        buffer
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}",
            self.render_styled(common::should_colorize_stderr())
        )
    }
}

impl std::error::Error for CliError {}
