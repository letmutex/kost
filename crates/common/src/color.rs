//! ANSI styling and terminal capability detection.

use std::ffi::OsStr;
use std::io::IsTerminal;

/// ANSI SGR escape sequence constants.
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const CYAN: &str = "\x1b[36m";
pub const BOLD_RED: &str = "\x1b[1;31m";
pub const BOLD_GREEN: &str = "\x1b[1;32m";
pub const BOLD_YELLOW: &str = "\x1b[1;33m";

/// Decides whether output should be colorized given terminal capability and environment values.
///
/// Follows the NO_COLOR specification (disabled only when present and non-empty),
/// and disables color when `TERM=dumb` or when output is not an interactive terminal.
pub fn decide_color(is_terminal: bool, no_color: Option<&OsStr>, term: Option<&str>) -> bool {
    if no_color.is_some_and(|val| !val.is_empty()) {
        return false;
    }
    if term == Some("dumb") {
        return false;
    }
    is_terminal
}

/// Evaluates environment variables to determine whether a given stream should be colorized.
pub fn should_colorize(is_terminal: bool) -> bool {
    let no_color = std::env::var_os("NO_COLOR");
    let term = std::env::var("TERM").ok();
    decide_color(is_terminal, no_color.as_deref(), term.as_deref())
}

/// Returns true if stdout should be colorized.
pub fn should_colorize_stdout() -> bool {
    should_colorize(std::io::stdout().is_terminal())
}

/// Returns true if stderr should be colorized.
pub fn should_colorize_stderr() -> bool {
    should_colorize(std::io::stderr().is_terminal())
}

/// Wraps text in an ANSI style when colorize is true.
pub fn wrap_styled(text: &str, ansi_code: &str, colorize: bool) -> String {
    if colorize {
        let mut styled = String::with_capacity(ansi_code.len() + text.len() + RESET.len());
        styled.push_str(ansi_code);
        styled.push_str(text);
        styled.push_str(RESET);
        styled
    } else {
        text.to_string()
    }
}
