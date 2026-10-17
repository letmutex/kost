use crate::error::CliError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Help,
    Version,
    LanguageServer,
    Update(Vec<String>),
    Lint(Vec<String>),
    Format(Vec<String>),
}

pub struct Flag {
    pub short: &'static str,
    pub long: &'static str,
    pub description: &'static str,
}

pub struct Subcommand {
    pub name: &'static str,
    pub description: &'static str,
}

pub struct MiniClap {
    pub name: &'static str,
    pub description: &'static str,
    pub flags: Vec<Flag>,
    pub subcommands: Vec<Subcommand>,
}

impl MiniClap {
    pub fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            flags: Vec::new(),
            subcommands: Vec::new(),
        }
    }

    pub fn flag(
        mut self,
        short: &'static str,
        long: &'static str,
        description: &'static str,
    ) -> Self {
        self.flags.push(Flag {
            short,
            long,
            description,
        });
        self
    }

    pub fn subcommand(mut self, name: &'static str, description: &'static str) -> Self {
        self.subcommands.push(Subcommand { name, description });
        self
    }

    pub fn parse(&self, arguments: &[String]) -> Result<CliCommand, CliError> {
        if arguments.is_empty() {
            return Ok(CliCommand::Help);
        }

        let first_argument = &arguments[0];
        match first_argument.as_str() {
            "-h" | "--help" => Ok(CliCommand::Help),
            "-v" | "--version" => Ok(CliCommand::Version),
            "-ls" | "--language-server" => Ok(CliCommand::LanguageServer),
            "update" => Ok(CliCommand::Update(arguments[1..].to_vec())),
            "lint" => Ok(CliCommand::Lint(arguments[1..].to_vec())),
            "fmt" | "format" => Ok(CliCommand::Format(arguments[1..].to_vec())),
            unrecognized if unrecognized.starts_with('-') => {
                Err(CliError::UnknownOption(unrecognized.to_string()))
            }
            unrecognized => Err(CliError::UnknownCommand(unrecognized.to_string())),
        }
    }

    pub fn render_help(&self) -> String {
        self.render_help_styled(common::should_colorize_stdout())
    }

    pub fn render_help_plain(&self) -> String {
        self.render_help_styled(false)
    }

    pub fn render_help_styled(&self, colorize: bool) -> String {
        let mut buffer = String::new();
        buffer.push_str(self.name);
        buffer.push_str(" - ");
        buffer.push_str(self.description);
        buffer.push_str("\n\n");

        crate::style::push_header(&mut buffer, "USAGE:", colorize);
        buffer.push_str("\n    ");
        buffer.push_str(self.name);
        buffer.push(' ');
        crate::style::push_placeholder(&mut buffer, "[OPTIONS]", colorize);
        buffer.push_str("\n    ");
        buffer.push_str(self.name);
        buffer.push(' ');
        crate::style::push_placeholder(&mut buffer, "<COMMAND>", colorize);
        buffer.push_str("\n\n");

        crate::style::push_header(&mut buffer, "OPTIONS:", colorize);
        buffer.push('\n');
        self.render_flags(&mut buffer, colorize);

        buffer.push('\n');
        crate::style::push_header(&mut buffer, "COMMANDS:", colorize);
        buffer.push('\n');
        self.render_subcommands(&mut buffer, colorize);

        buffer
    }

    fn render_flags(&self, buffer: &mut String, colorize: bool) {
        for flag in &self.flags {
            buffer.push_str("    ");
            if colorize {
                buffer.push_str(common::color::BLUE);
                buffer.push_str(flag.short);
                buffer.push_str(", ");
                buffer.push_str(flag.long);
                buffer.push_str(common::color::RESET);
            } else {
                buffer.push_str(flag.short);
                buffer.push_str(", ");
                buffer.push_str(flag.long);
            }

            let total_prefix_len = 6 + flag.short.len() + flag.long.len();
            let padding = if total_prefix_len < 29 {
                29 - total_prefix_len
            } else {
                1
            };
            for _ in 0..padding {
                buffer.push(' ');
            }
            buffer.push_str(flag.description);
            buffer.push('\n');
        }
    }

    fn render_subcommands(&self, buffer: &mut String, colorize: bool) {
        for subcmd in &self.subcommands {
            buffer.push_str("    ");
            crate::style::push_placeholder(buffer, subcmd.name, colorize);

            let total_prefix_len = 4 + subcmd.name.len();
            let padding = if total_prefix_len < 29 {
                29 - total_prefix_len
            } else {
                1
            };
            for _ in 0..padding {
                buffer.push(' ');
            }
            buffer.push_str(subcmd.description);
            buffer.push('\n');
        }
    }
}
