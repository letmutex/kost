//! Strict rule and condition parser according to architecture specification.

use crate::condition::{Condition, PropertyCondition};
use crate::config::error::ConfigError;
use crate::config::operator::{parse_operator, validate_property_operator};
use crate::types::{PropertyKey, PropertyValue, Scope};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Validates that a string is valid kebab-case (e.g. "no-semi", "long-function-1").
pub fn is_valid_kebab_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    for part in s.split('-') {
        if part.is_empty()
            || !part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return false;
        }
    }
    true
}

/// Parses a raw rule definition into validated options, condition, scope, and message.
pub struct RuleParser;

impl RuleParser {
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
            if !is_valid_kebab_case(key)
                && !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(ConfigError::InvalidIdFormat {
                    id: key.clone(),
                    reason: "Option key must be alphanumeric or snake_case/kebab-case",
                });
            }

            let typed_val = Self::yaml_value_to_property_value(key, value)?;
            parsed_options.insert(SmolStr::new(key), typed_val);
        }

        Ok(Some(parsed_options))
    }

    fn yaml_value_to_property_value(
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
                property: key.to_string(),
                operator: "options",
                expected: "Int, Float, Str, Bool, or StrList",
                actual: "Unsupported YAML type".to_string(),
            }),
        }
    }

    /// Parses and strictly validates a `where` clause into a `Condition` and target `Scope`.
    pub fn parse_where(
        rule_id: &str,
        raw_where: &serde_yaml::Value,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(Condition, Scope), ConfigError> {
        let mapping = raw_where
            .as_mapping()
            .ok_or_else(|| ConfigError::InvalidOperator {
                property: "where".to_string(),
                operator: "root".to_string(),
                reason: "'where' block must be a mapping",
            })?;

        if mapping.is_empty() {
            return Err(ConfigError::InvalidOperator {
                property: "where".to_string(),
                operator: "root".to_string(),
                reason: "'where' block cannot be empty",
            });
        }

        if mapping.len() > 1 {
            return Err(ConfigError::InvalidOperator {
                property: "where".to_string(),
                operator: "root".to_string(),
                reason: "'where' block must contain exactly one combinator ('all', 'any') or a single property condition",
            });
        }

        if let Some(all_val) = mapping.get(serde_yaml::Value::String("all".to_string())) {
            let seq = all_val
                .as_sequence()
                .ok_or_else(|| ConfigError::InvalidOperator {
                    property: "where.all".to_string(),
                    operator: "all".to_string(),
                    reason: "'all' must contain a sequence of conditions",
                })?;
            let (conditions, scope) = Self::parse_condition_sequence(rule_id, seq, options)?;
            Ok((Condition::All(conditions), scope))
        } else if let Some(any_val) = mapping.get(serde_yaml::Value::String("any".to_string())) {
            let seq = any_val
                .as_sequence()
                .ok_or_else(|| ConfigError::InvalidOperator {
                    property: "where.any".to_string(),
                    operator: "any".to_string(),
                    reason: "'any' must contain a sequence of conditions",
                })?;
            let (conditions, scope) = Self::parse_condition_sequence(rule_id, seq, options)?;
            Ok((Condition::Any(conditions), scope))
        } else {
            let (first_key, first_val) = mapping.iter().next().unwrap();
            let key_str = first_key
                .as_str()
                .ok_or_else(|| ConfigError::InvalidOperator {
                    property: "where".to_string(),
                    operator: "root".to_string(),
                    reason: "Property key must be a string",
                })?;
            let (prop_cond, scope) =
                Self::parse_property_condition(rule_id, key_str, first_val, options)?;
            Ok((Condition::Single(prop_cond), scope))
        }
    }

    fn parse_condition_sequence(
        rule_id: &str,
        seq: &[serde_yaml::Value],
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(Vec<PropertyCondition>, Scope), ConfigError> {
        if seq.is_empty() {
            return Err(ConfigError::InvalidOperator {
                property: "where".to_string(),
                operator: "combinator".to_string(),
                reason: "Condition sequence cannot be empty",
            });
        }

        let mut conditions = Vec::with_capacity(seq.len());

        let mut inferred_scope: Option<Scope> = None;

        for item in seq {
            let (prop_cond, scope) = Self::parse_single_condition_value(rule_id, item, options)?;
            if let Some(existing_scope) = inferred_scope {
                if existing_scope != scope {
                    return Err(ConfigError::ScopeMismatch {
                        rule_id: rule_id.to_string(),
                        expected_scope: existing_scope.as_str(),
                        actual_scope: scope.as_str(),
                        property: prop_cond.property.field_name().to_string(),
                    });
                }
            } else {
                inferred_scope = Some(scope);
            }
            conditions.push(prop_cond);
        }

        let scope = inferred_scope.unwrap_or(Scope::File);
        Ok((conditions, scope))
    }

    fn parse_single_condition_value(
        rule_id: &str,
        value: &serde_yaml::Value,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(PropertyCondition, Scope), ConfigError> {
        let mapping = value
            .as_mapping()
            .ok_or_else(|| ConfigError::InvalidOperator {
                property: "condition".to_string(),
                operator: "item".to_string(),
                reason: "Condition item must be a mapping of '<scope>.<property>: <op>'",
            })?;

        if mapping.is_empty() {
            return Err(ConfigError::InvalidOperator {
                property: "condition".to_string(),
                operator: "item".to_string(),
                reason: "Condition item cannot be empty",
            });
        }

        if mapping.len() > 1 {
            return Err(ConfigError::InvalidOperator {
                property: "condition".to_string(),
                operator: "item".to_string(),
                reason: "Condition item must contain exactly one '<scope>.<property>: <op>' pair",
            });
        }

        let (key_val, op_val) = mapping.iter().next().unwrap();

        let key_str = key_val
            .as_str()
            .ok_or_else(|| ConfigError::InvalidOperator {
                property: "condition".to_string(),
                operator: "key".to_string(),
                reason: "Property key must be a string",
            })?;

        Self::parse_property_condition(rule_id, key_str, op_val, options)
    }

    fn parse_property_condition(
        rule_id: &str,
        property_path: &str,
        op_val: &serde_yaml::Value,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(PropertyCondition, Scope), ConfigError> {
        let (scope_str, field_str) =
            property_path
                .split_once('.')
                .ok_or_else(|| ConfigError::InvalidOperator {
                    property: property_path.to_string(),
                    operator: "path".to_string(),
                    reason: "Property must be scoped in '<scope>.<property>' format",
                })?;

        let scope = Scope::parse(scope_str).ok_or_else(|| ConfigError::UnknownScope {
            scope: scope_str.to_string(),
            property: property_path.to_string(),
        })?;

        let property_key =
            PropertyKey::from_scope_and_field(scope, field_str).ok_or_else(|| {
                ConfigError::UnknownProperty {
                    scope: scope_str.to_string(),
                    property: field_str.to_string(),
                }
            })?;

        let op = parse_operator(rule_id, property_path, op_val, options)?;
        validate_property_operator(property_key, property_path, &op, options)?;
        Ok((PropertyCondition::new(property_key, op), scope))
    }

    /// Strictly validates message template placeholders (`${scope.property}` and `${options.key}`).
    #[inline]
    pub fn validate_message_template(
        rule_scope: Scope,
        template: &str,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(), ConfigError> {
        crate::config::template::validate_message_template(rule_scope, template, options)
    }
}
