//! Interpretation and application of EditorConfig properties onto ResolvedEditorConfig.

use smol_str::SmolStr;

use crate::editorconfig::model::{
    EndOfLine, IndentStyle, KtlintCodeStyle, LineLengthSetting, ResolvedEditorConfig,
    TrailingCommaStrategy,
};
use crate::editorconfig::parser::EditorConfigFile;

/// Applies properties from sections of a file that match the target's relative path.
pub fn apply_file_properties(
    file: &EditorConfigFile,
    relative_path: &str,
    target: &mut ResolvedEditorConfig,
) {
    for section in &file.sections {
        if section.matcher.is_match(relative_path) {
            for (key, val) in &section.properties {
                apply_single_property(key, val, target);
            }
        }
    }
}

pub(crate) fn apply_single_property(
    key: &SmolStr,
    val: &SmolStr,
    target: &mut ResolvedEditorConfig,
) {
    let key_str = key.as_str();
    let val_str = val.as_str();

    if val_str.eq_ignore_ascii_case("unset") {
        unset_property(key_str, target);
        return;
    }

    target
        .raw_properties
        .insert(key.clone(), SmolStr::new(val_str));

    match key_str {
        "indent_style" => update_some(&mut target.indent_style, IndentStyle::parse(val_str)),
        "indent_size" => {
            if val_str.eq_ignore_ascii_case("tab") {
                target.indent_size = target.tab_width.or(Some(4));
            } else if let Ok(n) = val_str.parse::<usize>() {
                target.indent_size = Some(n);
            }
        }
        "tab_width" => {
            if let Ok(n) = val_str.parse::<usize>() {
                target.tab_width = Some(n);
                if target
                    .raw_properties
                    .get("indent_size")
                    .is_some_and(|v| v.eq_ignore_ascii_case("tab"))
                {
                    target.indent_size = Some(n);
                }
            }
        }
        "max_line_length" => {
            update_some(
                &mut target.max_line_length,
                LineLengthSetting::parse(val_str),
            );
        }
        "insert_final_newline" => {
            update_some(
                &mut target.insert_final_newline,
                parse_boolean_value(val_str),
            );
        }
        "end_of_line" => update_some(&mut target.end_of_line, EndOfLine::parse(val_str)),
        "trim_trailing_whitespace" => {
            update_some(
                &mut target.trim_trailing_whitespace,
                parse_boolean_value(val_str),
            );
        }
        "ij_kotlin_indent_size" => {
            update_some(&mut target.ij_kotlin_indent_size, val_str.parse().ok());
        }
        "ij_continuation_indent_size"
        | "ij_kotlin_continuation_indent_size"
        | "continuation_indent_size" => {
            update_some(
                &mut target.ij_continuation_indent_size,
                val_str.parse().ok(),
            );
        }
        "ij_kotlin_allow_trailing_comma" => {
            update_some(
                &mut target.ij_allow_trailing_comma,
                parse_boolean_value(val_str),
            );
        }
        "ij_kotlin_allow_trailing_comma_on_call_site" => {
            update_some(
                &mut target.ij_allow_trailing_comma_on_call_site,
                parse_boolean_value(val_str),
            );
        }
        "ktfmt_trailing_comma_management_strategy" => {
            update_some(
                &mut target.ktfmt_trailing_comma_strategy,
                TrailingCommaStrategy::parse(val_str),
            );
        }
        "ktfmt_style" => target.ktfmt_style = Some(SmolStr::new(val_str.to_ascii_lowercase())),
        "ktlint_code_style" => {
            update_some(
                &mut target.ktlint_code_style,
                KtlintCodeStyle::parse(val_str),
            );
        }
        other if let Some(toggle) = parse_ktlint_toggle_key(other) => {
            if let Some(enabled) = parse_enabled_disabled(val_str) {
                match toggle {
                    KtlintToggle::Global => {
                        target.ktlint_enabled = Some(enabled);
                    }
                    KtlintToggle::Ruleset(ruleset) => {
                        if ruleset == "standard" {
                            target.ktlint_standard_enabled = Some(enabled);
                        }
                        target.ruleset_toggles.insert(ruleset, enabled);
                    }
                    KtlintToggle::Rule { canonical, legacy } => {
                        target.rule_toggles.insert(canonical, enabled);
                        target.rule_toggles.insert(legacy, enabled);
                    }
                }
            }
        }
        _ => {}
    }
}

#[inline]
fn update_some<T>(slot: &mut Option<T>, value: Option<T>) {
    if let Some(v) = value {
        *slot = Some(v);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum KtlintToggle {
    Global,
    Ruleset(SmolStr),
    Rule { canonical: SmolStr, legacy: SmolStr },
}

fn parse_ktlint_toggle_key(key: &str) -> Option<KtlintToggle> {
    if key == "ktlint" {
        return Some(KtlintToggle::Global);
    }
    let remainder = key.strip_prefix("ktlint_")?;
    if remainder.is_empty() || remainder == "code_style" {
        return None;
    }
    if let Some((ruleset, rule_id)) = remainder.split_once('_') {
        if ruleset.is_empty() || rule_id.is_empty() {
            return None;
        }
        let normalized_id = rule_id.replace('_', "-");
        let canonical = crate::concat_smol_str!(ruleset, ":", &normalized_id);
        let legacy = crate::concat_smol_str!("ktlint_", ruleset, "_", &normalized_id);
        Some(KtlintToggle::Rule { canonical, legacy })
    } else {
        Some(KtlintToggle::Ruleset(SmolStr::new(remainder)))
    }
}

fn unset_property(key: &str, target: &mut ResolvedEditorConfig) {
    target.raw_properties.remove(key);
    match key {
        "indent_style" => target.indent_style = None,
        "indent_size" => target.indent_size = None,
        "tab_width" => target.tab_width = None,
        "max_line_length" => target.max_line_length = None,
        "insert_final_newline" => target.insert_final_newline = None,
        "end_of_line" => target.end_of_line = None,
        "trim_trailing_whitespace" => target.trim_trailing_whitespace = None,
        "ij_kotlin_indent_size" => target.ij_kotlin_indent_size = None,
        "ij_continuation_indent_size"
        | "ij_kotlin_continuation_indent_size"
        | "continuation_indent_size" => target.ij_continuation_indent_size = None,
        "ij_kotlin_allow_trailing_comma" => target.ij_allow_trailing_comma = None,
        "ij_kotlin_allow_trailing_comma_on_call_site" => {
            target.ij_allow_trailing_comma_on_call_site = None;
        }
        "ktfmt_trailing_comma_management_strategy" => {
            target.ktfmt_trailing_comma_strategy = None;
        }
        "ktfmt_style" => target.ktfmt_style = None,
        "ktlint_code_style" => target.ktlint_code_style = None,
        other if let Some(toggle) = parse_ktlint_toggle_key(other) => match toggle {
            KtlintToggle::Global => {
                target.ktlint_enabled = None;
            }
            KtlintToggle::Ruleset(ruleset) => {
                if ruleset == "standard" {
                    target.ktlint_standard_enabled = None;
                }
                target.ruleset_toggles.remove(&ruleset);
            }
            KtlintToggle::Rule { canonical, legacy } => {
                target.rule_toggles.remove(&canonical);
                target.rule_toggles.remove(&legacy);
            }
        },
        _ => {}
    }
}

fn parse_boolean_value(val: &str) -> Option<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_enabled_disabled(val: &str) -> Option<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "enabled" | "true" | "1" => Some(true),
        "disabled" | "false" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::apply_single_property;
    use crate::editorconfig::model::{EndOfLine, ResolvedEditorConfig};
    use smol_str::SmolStr;

    #[test]
    fn test_invalid_property_value_does_not_overwrite_inherited() {
        let mut config = ResolvedEditorConfig::new();
        config.end_of_line = Some(EndOfLine::Crlf);
        config.insert_final_newline = Some(true);

        apply_single_property(
            &SmolStr::new("end_of_line"),
            &SmolStr::new("invalid_eol"),
            &mut config,
        );
        apply_single_property(
            &SmolStr::new("insert_final_newline"),
            &SmolStr::new("not_a_bool"),
            &mut config,
        );

        assert_eq!(config.end_of_line, Some(EndOfLine::Crlf));
        assert_eq!(config.insert_final_newline, Some(true));
    }
}
