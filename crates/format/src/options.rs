/// Trailing comma management strategy, matching ktfmt's TrailingCommaManagementStrategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingCommaStrategy {
    /// Do not manage trailing commas; only adjust spacing around existing ones.
    None,
    /// Insert trailing commas when lists wrap across lines, but do not strip user-written commas.
    #[default]
    OnlyAdd,
    /// Insert trailing commas when multiline, and strip trailing commas when elements fit on a single line.
    Complete,
}

/// Kotlin source file type (syntactic property of the source, not style options)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileType {
    /// Standard Kotlin source file (.kt)
    #[default]
    Regular,
    /// Kotlin script (.kts)
    Script,
}

/// Style preset configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StylePreset {
    /// Meta style: block 2, continuation 4, OnlyAdd commas, max width 100
    Meta,
    /// Google style: block 2, continuation 2, Complete commas, max width 100
    Google,
    /// Kotlinlang style (default): block 4, continuation 4, Complete commas, exploded parens, max width 100
    #[default]
    Kotlinlang,
}

/// Comprehensive formatting options with a fluent builder pattern.
/// Uses `#[non_exhaustive]` so adding future options does not break downstream call sites.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattingOptions {
    pub max_width: usize,
    pub block_indent: usize,
    pub continuation_indent: usize,
    pub trailing_comma_strategy: TrailingCommaStrategy,
    pub remove_unused_imports: bool,
    pub preserve_lambda_breaks: bool,
    pub end_of_line: Option<common::editorconfig::EndOfLine>,
    pub insert_final_newline: bool,
}

impl FormattingOptions {
    pub const DEFAULT_MAX_WIDTH: usize = 100;
    pub const DEFAULT_BLOCK_INDENT: usize = 2;
    pub const DEFAULT_CONTINUATION_INDENT: usize = 4;

    /// Meta style preset
    pub fn meta() -> Self {
        Self {
            max_width: Self::DEFAULT_MAX_WIDTH,
            block_indent: 2,
            continuation_indent: 4,
            trailing_comma_strategy: TrailingCommaStrategy::OnlyAdd,
            remove_unused_imports: true,
            preserve_lambda_breaks: true,
            end_of_line: None,
            insert_final_newline: true,
        }
    }

    /// Google style preset
    pub fn google() -> Self {
        Self {
            max_width: Self::DEFAULT_MAX_WIDTH,
            block_indent: 2,
            continuation_indent: 2,
            trailing_comma_strategy: TrailingCommaStrategy::Complete,
            remove_unused_imports: true,
            preserve_lambda_breaks: true,
            end_of_line: None,
            insert_final_newline: true,
        }
    }

    /// Kotlinlang style preset
    pub fn kotlinlang() -> Self {
        Self {
            max_width: Self::DEFAULT_MAX_WIDTH,
            block_indent: 4,
            continuation_indent: 4,
            trailing_comma_strategy: TrailingCommaStrategy::Complete,
            remove_unused_imports: true,
            preserve_lambda_breaks: true,
            end_of_line: None,
            insert_final_newline: true,
        }
    }

    /// Derives formatting options by overlaying resolved EditorConfig properties on a base preset.
    pub fn from_editorconfig(
        base_preset: StylePreset,
        config: &common::editorconfig::ResolvedEditorConfig,
    ) -> Self {
        let preset = match config.ktfmt_style.as_deref() {
            Some("meta") => StylePreset::Meta,
            Some("google") => StylePreset::Google,
            Some("kotlinlang") => StylePreset::Kotlinlang,
            _ => base_preset,
        };
        let mut options = Self::from_preset(preset);

        if let Some(indent) = config.effective_indent_size() {
            options.block_indent = indent;
        }
        if let Some(cont_indent) = config.ij_continuation_indent_size {
            options.continuation_indent = cont_indent;
        }
        if let Some(line_length) = config.max_line_length {
            match line_length {
                common::editorconfig::LineLengthSetting::Value(len) => options.max_width = len,
                common::editorconfig::LineLengthSetting::Off => options.max_width = usize::MAX,
            }
        }
        if let Some(strategy) = config.ktfmt_trailing_comma_strategy {
            options.trailing_comma_strategy = strategy.into();
        } else if !config.allow_trailing_comma_on_declaration_site()
            && !config.allow_trailing_comma_on_call_site()
        {
            options.trailing_comma_strategy = TrailingCommaStrategy::None;
        }
        if let Some(insert_newline) = config.insert_final_newline {
            options.insert_final_newline = insert_newline;
        }
        if let Some(eol) = config.end_of_line {
            options.end_of_line = Some(eol);
        }

        options
    }

    /// Construct options from a given preset
    pub fn from_preset(preset: StylePreset) -> Self {
        match preset {
            StylePreset::Meta => Self::meta(),
            StylePreset::Google => Self::google(),
            StylePreset::Kotlinlang => Self::kotlinlang(),
        }
    }

    pub fn with_max_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    pub fn with_block_indent(mut self, indent: usize) -> Self {
        self.block_indent = indent;
        self
    }

    pub fn with_continuation_indent(mut self, indent: usize) -> Self {
        self.continuation_indent = indent;
        self
    }

    pub fn with_trailing_comma_strategy(mut self, strategy: TrailingCommaStrategy) -> Self {
        self.trailing_comma_strategy = strategy;
        self
    }

    pub fn with_remove_unused_imports(mut self, remove: bool) -> Self {
        self.remove_unused_imports = remove;
        self
    }

    pub fn with_preserve_lambda_breaks(mut self, preserve: bool) -> Self {
        self.preserve_lambda_breaks = preserve;
        self
    }
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self::kotlinlang()
    }
}

impl From<StylePreset> for FormattingOptions {
    fn from(preset: StylePreset) -> Self {
        Self::from_preset(preset)
    }
}

impl From<cli::FormatStyle> for StylePreset {
    fn from(style: cli::FormatStyle) -> Self {
        match style {
            cli::FormatStyle::Kotlinlang => Self::Kotlinlang,
            cli::FormatStyle::Meta => Self::Meta,
            cli::FormatStyle::Google => Self::Google,
        }
    }
}

impl From<common::editorconfig::TrailingCommaStrategy> for TrailingCommaStrategy {
    fn from(strategy: common::editorconfig::TrailingCommaStrategy) -> Self {
        match strategy {
            common::editorconfig::TrailingCommaStrategy::None => Self::None,
            common::editorconfig::TrailingCommaStrategy::OnlyAdd => Self::OnlyAdd,
            common::editorconfig::TrailingCommaStrategy::Complete => Self::Complete,
        }
    }
}
