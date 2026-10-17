//! Kotlin code formatting engine for kost.

pub mod check;
pub mod cli;
pub mod diff;
pub mod doc;
pub mod engine;
pub mod error;
pub mod formatter;
pub mod lower;
pub mod options;
pub mod printer;
pub mod rules;
pub mod smart_diff;
pub mod util;

pub use cli::run;
pub use diff::create_diff;
pub use error::{FormatError, Position};
pub use formatter::{CheckResult, Formatter};
pub use options::{FileType, FormattingOptions, StylePreset, TrailingCommaStrategy};

/// Format Kotlin regular source code using given options.
pub fn format(source: &str, options: &FormattingOptions) -> Result<String, FormatError> {
    format_file(source, FileType::Regular, options)
}

/// Format Kotlin source code with explicit FileType (Regular vs Script).
///
/// Strips UTF-8 BOM, normalizes input newlines to '\n' for engine processing,
/// and restores CRLF if present in original input.
pub fn format_file(
    source: &str,
    file_type: FileType,
    options: &FormattingOptions,
) -> Result<String, FormatError> {
    // 1. Strip leading UTF-8 BOM if present
    let clean_source = source.strip_prefix('\u{feff}').unwrap_or(source);

    // 2. Normalize input to '\n' to ensure consistent AST offsets and column measuring
    let normalized = if clean_source.contains("\r\n") {
        std::borrow::Cow::Owned(clean_source.replace("\r\n", "\n"))
    } else if clean_source.contains('\r') {
        std::borrow::Cow::Owned(clean_source.replace('\r', "\n"))
    } else {
        std::borrow::Cow::Borrowed(clean_source)
    };

    // 3. Auto-detect script if shebang present strictly on line 1
    let resolved_file_type = if normalized.starts_with("#!") {
        FileType::Script
    } else {
        file_type
    };

    // 4. Execute internal engine with normalized '\n'
    let mut formatted_normalized = engine::format_source(&normalized, resolved_file_type, options)?;

    if !options.insert_final_newline {
        while formatted_normalized.ends_with('\n') || formatted_normalized.ends_with('\r') {
            formatted_normalized.pop();
        }
    }

    // 5. Restore target line endings (honoring explicit config or input detection)
    match options.end_of_line {
        Some(common::editorconfig::EndOfLine::Crlf) => {
            Ok(formatted_normalized.replace('\n', "\r\n"))
        }
        Some(common::editorconfig::EndOfLine::Cr) => Ok(formatted_normalized.replace('\n', "\r")),
        Some(common::editorconfig::EndOfLine::Lf) => Ok(formatted_normalized),
        None if clean_source.contains("\r\n") => Ok(formatted_normalized.replace('\n', "\r\n")),
        None => Ok(formatted_normalized),
    }
}

/// Format Kotlin source code using default Kotlinlang style.
pub fn format_default(source: &str) -> Result<String, FormatError> {
    format(source, &FormattingOptions::default())
}
