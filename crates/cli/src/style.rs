use common::color::{BOLD_GREEN, CYAN, RESET, wrap_styled};

/// Returns the error prefix tag (`error:`) with optional bold red ANSI styling.
pub fn error_prefix(colorize: bool) -> &'static str {
    if colorize {
        "\x1b[1;31merror:\x1b[0m"
    } else {
        "error:"
    }
}

/// Returns the help flag option (`--help`) with optional cyan ANSI styling.
pub fn help_option(colorize: bool) -> &'static str {
    if colorize {
        "\x1b[36m--help\x1b[0m"
    } else {
        "--help"
    }
}

/// Appends a styled section header (bold green) to the target string buffer.
pub fn push_header(buffer: &mut String, header_text: &str, colorize: bool) {
    if colorize {
        buffer.push_str(BOLD_GREEN);
        buffer.push_str(header_text);
        buffer.push_str(RESET);
    } else {
        buffer.push_str(header_text);
    }
}

/// Appends a styled option flag (cyan) to the target string buffer.
pub fn push_option(buffer: &mut String, option_text: &str, colorize: bool) {
    if colorize {
        buffer.push_str(CYAN);
        buffer.push_str(option_text);
        buffer.push_str(RESET);
    } else {
        buffer.push_str(option_text);
    }
}

/// Appends a styled positional placeholder or command (cyan) to the target string buffer.
pub fn push_placeholder(buffer: &mut String, placeholder_text: &str, colorize: bool) {
    if colorize {
        buffer.push_str(CYAN);
        buffer.push_str(placeholder_text);
        buffer.push_str(RESET);
    } else {
        buffer.push_str(placeholder_text);
    }
}

/// Styles header text in bold green.
pub fn style_header(text: &str, colorize: bool) -> String {
    wrap_styled(text, BOLD_GREEN, colorize)
}

/// Styles option text in cyan.
pub fn style_option(text: &str, colorize: bool) -> String {
    wrap_styled(text, CYAN, colorize)
}

/// Styles placeholder or command text in cyan.
pub fn style_placeholder(text: &str, colorize: bool) -> String {
    wrap_styled(text, CYAN, colorize)
}
