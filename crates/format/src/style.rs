use crate::options::{FormattingOptions, StylePreset};

/// Semantic formatting style rules decoupled from raw indent numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleProfile {
    pub preset: StylePreset,
    /// Break after annotations on declarations (Kotlinlang vs Meta / Google)
    pub break_after_declaration_annotations: bool,
    /// Wrap condition expressions inside `if (...)`, `while (...)`, `when (...)` parens
    pub soft_break_inside_condition_parens: bool,
    /// Keep `.trimIndent()` / `.trimMargin()` on same line as closing `"""`
    pub attach_raw_string_trim_call: bool,
    /// Allow top-level / destructuring unchained calls with multiline args to keep `(` on `=` line
    pub allow_flat_multiline_call_on_rhs: bool,
    /// Indent trailing lambda extra when preceding value_arguments break
    pub indent_trailing_lambda_after_broken_args: bool,
    /// Omit closing softline before `)` when last arg is a block/brace and no trailing comma
    pub pack_closing_paren_after_block_arg: bool,
    /// Align wrapped trailing line comments to the start column of the trailing comment
    pub align_wrapped_trailing_comments: bool,
}

impl StyleProfile {
    pub fn for_preset(preset: StylePreset) -> Self {
        match preset {
            StylePreset::Kotlinlang => Self {
                preset: StylePreset::Kotlinlang,
                break_after_declaration_annotations: true,
                soft_break_inside_condition_parens: true,
                attach_raw_string_trim_call: true,
                allow_flat_multiline_call_on_rhs: true,
                indent_trailing_lambda_after_broken_args: false,
                pack_closing_paren_after_block_arg: false,
                align_wrapped_trailing_comments: true,
            },
            StylePreset::Google => Self {
                preset: StylePreset::Google,
                break_after_declaration_annotations: false,
                soft_break_inside_condition_parens: true,
                attach_raw_string_trim_call: false,
                allow_flat_multiline_call_on_rhs: false,
                indent_trailing_lambda_after_broken_args: false,
                pack_closing_paren_after_block_arg: true,
                align_wrapped_trailing_comments: true,
            },
            StylePreset::Meta => Self {
                preset: StylePreset::Meta,
                break_after_declaration_annotations: false,
                soft_break_inside_condition_parens: false,
                attach_raw_string_trim_call: false,
                allow_flat_multiline_call_on_rhs: false,
                indent_trailing_lambda_after_broken_args: true,
                pack_closing_paren_after_block_arg: true,
                align_wrapped_trailing_comments: true,
            },
        }
    }

    pub fn from_options(options: &FormattingOptions) -> Self {
        let preset = match options.style_preset {
            Some(p) => p,
            None => {
                if options.block_indent == 4 {
                    StylePreset::Kotlinlang
                } else if options.continuation_indent == 2 {
                    StylePreset::Google
                } else {
                    StylePreset::Meta
                }
            }
        };
        Self::for_preset(preset)
    }
}
