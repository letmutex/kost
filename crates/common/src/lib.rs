//! Common abstractions and shared utilities for kost.

pub mod color;
pub mod editorconfig;
pub mod files;
pub mod util;

pub use color::{decide_color, should_colorize, should_colorize_stderr, should_colorize_stdout};
pub use files::{is_kotlin_file, resolve_target_files};
pub use util::format_elapsed;
