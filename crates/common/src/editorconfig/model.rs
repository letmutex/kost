//! Strongly-typed domain model representing resolved EditorConfig properties.

use ahash::AHashMap;
use smol_str::SmolStr;

/// Standard indentation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndentStyle {
    #[default]
    Space,
    Tab,
}

impl IndentStyle {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "space" => Some(Self::Space),
            "tab" => Some(Self::Tab),
            _ => None,
        }
    }
}

/// Standard line ending specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndOfLine {
    #[default]
    Lf,
    Crlf,
    Cr,
}

impl EndOfLine {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "lf" => Some(Self::Lf),
            "crlf" => Some(Self::Crlf),
            "cr" => Some(Self::Cr),
            _ => None,
        }
    }
}

/// Maximum line length configuration: either a positive limit or disabled (`off`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineLengthSetting {
    Value(usize),
    Off,
}

impl LineLengthSetting {
    pub fn parse(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("off") {
            Some(Self::Off)
        } else if let Ok(parsed) = trimmed.parse::<usize>() {
            if parsed > 0 {
                Some(Self::Value(parsed))
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Trailing comma management strategy matching ktfmt and ktlint expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrailingCommaStrategy {
    None,
    #[default]
    OnlyAdd,
    Complete,
}

impl TrailingCommaStrategy {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "only_add" => Some(Self::OnlyAdd),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

/// Ktlint code style preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KtlintCodeStyle {
    #[default]
    Official,
    IntellijIdea,
    AndroidStudio,
}

impl KtlintCodeStyle {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ktlint_official" | "official" => Some(Self::Official),
            "intellij_idea" | "idea" => Some(Self::IntellijIdea),
            "android_studio" | "android" => Some(Self::AndroidStudio),
            _ => None,
        }
    }
}

/// Fully resolved EditorConfig configuration for a specific target file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedEditorConfig {
    pub indent_style: Option<IndentStyle>,
    pub indent_size: Option<usize>,
    pub tab_width: Option<usize>,
    pub max_line_length: Option<LineLengthSetting>,
    pub insert_final_newline: Option<bool>,
    pub end_of_line: Option<EndOfLine>,
    pub trim_trailing_whitespace: Option<bool>,

    pub ij_kotlin_indent_size: Option<usize>,
    pub ij_continuation_indent_size: Option<usize>,
    pub ij_allow_trailing_comma: Option<bool>,
    pub ij_allow_trailing_comma_on_call_site: Option<bool>,

    pub ktfmt_style: Option<SmolStr>,
    pub ktfmt_trailing_comma_strategy: Option<TrailingCommaStrategy>,

    pub ktlint_code_style: Option<KtlintCodeStyle>,
    pub ktlint_standard_enabled: Option<bool>,
    pub rule_toggles: AHashMap<SmolStr, bool>,
    pub raw_properties: AHashMap<SmolStr, SmolStr>,
}

impl ResolvedEditorConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates whether a lint rule is enabled based on specific or ruleset-level toggles.
    pub fn is_rule_enabled(&self, rule_id: &str, default_enabled: bool) -> bool {
        let clean_id = rule_id.strip_prefix("standard:").unwrap_or(rule_id);
        let normalized_suffix = clean_id.replace('_', "-");
        let specific_toggle_key = crate::concat_smol_str!("ktlint_standard_", &normalized_suffix);

        if let Some(&enabled) = self.rule_toggles.get(&specific_toggle_key) {
            return enabled;
        }

        if let Some(standard_enabled) = self.ktlint_standard_enabled {
            return standard_enabled;
        }

        default_enabled
    }

    /// Returns whether trailing commas are allowed on declaration sites.
    #[inline]
    pub fn allow_trailing_comma_on_declaration_site(&self) -> bool {
        self.ij_allow_trailing_comma.unwrap_or(true)
    }

    /// Returns whether trailing commas are allowed on call sites (falls back to declaration-site setting).
    #[inline]
    pub fn allow_trailing_comma_on_call_site(&self) -> bool {
        self.ij_allow_trailing_comma_on_call_site
            .or(self.ij_allow_trailing_comma)
            .unwrap_or(true)
    }

    /// Returns the effective block indent size (prioritizing `ij_kotlin_indent_size` over `indent_size`).
    pub fn effective_indent_size(&self) -> Option<usize> {
        self.ij_kotlin_indent_size.or(self.indent_size)
    }
}
