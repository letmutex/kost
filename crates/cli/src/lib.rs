pub mod error;
pub mod format;
pub mod lint;
pub mod mini_clap;
pub mod print_mode;
pub mod style;
pub mod update;

pub use error::CliError;
pub use format::{FormatArgs, FormatStyle, render_format_help, render_format_help_styled};
pub use lint::{LintArgs, render_lint_help, render_lint_help_styled};
pub use mini_clap::{CliCommand, MiniClap};
pub use print_mode::PrintMode;
pub use update::{UpdateArgs, render_update_help, render_update_help_styled};

pub fn build_app() -> MiniClap {
    MiniClap::new(
        "kost",
        "Fast JVM-free Kotlin static analysis & language server",
    )
    .flag("-h", "--help", "Print help information")
    .flag("-v", "--version", "Print version information")
    .flag(
        "-ls",
        "--language-server",
        "Start the Language Server Protocol (LSP) server over stdio",
    )
    .subcommand(
        "update",
        "Update the kost CLI binary to the latest or specified release",
    )
    .subcommand("lint", "Lint Kotlin source files")
    .subcommand("fmt", "Format Kotlin source files")
}

pub fn parse_args(arguments: &[String]) -> Result<CliCommand, CliError> {
    build_app().parse(arguments)
}

pub fn print_help() {
    print!("{}", build_app().render_help());
}

pub fn print_version() {
    println!("kost {}", env!("CARGO_PKG_VERSION"));
}

pub(crate) fn parse_value_option<'a, I>(
    arg: &str,
    prefix: &str,
    iter: &mut I,
) -> Result<Option<String>, CliError>
where
    I: Iterator<Item = &'a String>,
{
    if let Some(val) = arg.strip_prefix(prefix) {
        if let Some(stripped) = val.strip_prefix('=') {
            if stripped.trim().is_empty() {
                return Err(CliError::MissingOptionValue(prefix.to_string()));
            }
            return Ok(Some(stripped.to_string()));
        } else if val.is_empty() {
            match iter.next() {
                Some(next_val)
                    if next_val == "--"
                        || next_val.starts_with('-')
                        || next_val.trim().is_empty() =>
                {
                    return Err(CliError::MissingOptionValue(prefix.to_string()));
                }
                Some(next_val) => return Ok(Some(next_val.to_string())),
                None => return Err(CliError::MissingOptionValue(prefix.to_string())),
            }
        } else if !prefix.starts_with("--") {
            let trimmed = val.trim();
            if trimmed.is_empty() {
                return Err(CliError::MissingOptionValue(prefix.to_string()));
            }
            return Ok(Some(trimmed.to_string()));
        }
    }
    Ok(None)
}
