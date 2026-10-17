//! EditorConfig support for Kotlin code formatting and linting.

pub mod glob;
pub mod model;
pub mod parser;
pub mod property;
pub mod resolver;

pub use glob::compile_glob;
pub use model::{
    EndOfLine, IndentStyle, KtlintCodeStyle, LineLengthSetting, ResolvedEditorConfig,
    TrailingCommaStrategy,
};
pub use parser::{EditorConfigFile, EditorConfigSection};
pub use property::apply_file_properties;
pub use resolver::EditorConfigResolver;
