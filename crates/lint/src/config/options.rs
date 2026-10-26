//! Parser and converter for raw YAML rule options.

use crate::config::error::ConfigError;
use crate::types::PropertyValue;
use common::util::is_valid_kebab_case;
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Validates and converts raw YAML options into typed `IndexMap<SmolStr, PropertyValue>`.
pub fn parse_options(
    rule_id: &str,
    raw_options: &Option<IndexMap<String, serde_yaml::Value>>,
) -> Result<Option<IndexMap<SmolStr, PropertyValue>>, ConfigError> {
    let Some(raw_map) = raw_options else {
        return Ok(None);
    };

    if raw_map.is_empty() {
        return Err(ConfigError::EmptyOptions {
            rule_id: rule_id.to_string(),
        });
    }

    let mut parsed_options = IndexMap::with_capacity(raw_map.len());
    for (key, value) in raw_map {
        if key.is_empty()
            || (!is_valid_kebab_case(key)
                && !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            return Err(ConfigError::InvalidOptionKey {
                rule_id: rule_id.to_string(),
                option_key: key.clone(),
                reason: "Option key must be alphanumeric or snake_case/kebab-case",
            });
        }

        let typed_val = parse_option_value(rule_id, key, value)?;
        parsed_options.insert(SmolStr::new(key), typed_val);
    }

    Ok(Some(parsed_options))
}

/// Converts a single raw YAML option value into a typed `PropertyValue`.
pub fn parse_option_value(
    rule_id: &str,
    key: &str,
    value: &serde_yaml::Value,
) -> Result<PropertyValue, ConfigError> {
    match value {
        serde_yaml::Value::Number(num) => {
            if let Some(int_val) = num.as_i64() {
                Ok(PropertyValue::Int(int_val))
            } else if let Some(float_val) = num.as_f64() {
                Ok(PropertyValue::Float(float_val))
            } else {
                Err(ConfigError::InvalidOperandType {
                    rule_id: rule_id.to_string(),
                    property: key.to_string(),
                    operator: "options",
                    expected: "Int or Float",
                    actual: "Invalid number".to_string(),
                })
            }
        }
        serde_yaml::Value::String(s) => Ok(PropertyValue::Str(SmolStr::new(s))),
        serde_yaml::Value::Bool(b) => Ok(PropertyValue::Bool(*b)),
        serde_yaml::Value::Sequence(seq) => {
            let mut list = SmallVec::new();
            for item in seq {
                if let Some(s) = item.as_str() {
                    list.push(SmolStr::new(s));
                } else {
                    return Err(ConfigError::InvalidOperandType {
                        rule_id: rule_id.to_string(),
                        property: key.to_string(),
                        operator: "options",
                        expected: "String items",
                        actual: "Non-string item".to_string(),
                    });
                }
            }
            Ok(PropertyValue::StrList(list))
        }
        _ => Err(ConfigError::InvalidOperandType {
            rule_id: rule_id.to_string(),
            property: key.to_string(),
            operator: "options",
            expected: "Int, Float, Str, Bool, or StrList",
            actual: "Unsupported YAML type".to_string(),
        }),
    }
}
