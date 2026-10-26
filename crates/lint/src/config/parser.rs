//! Strict rule and condition parser according to architecture specification.

use crate::condition::{Condition, PropertyCondition};
use crate::config::error::ConfigError;
use crate::config::operator::{parse_operator, validate_property_operator};
use crate::config::options;
use crate::types::{PropertyKey, PropertyValue, Scope};
use indexmap::IndexMap;
use smol_str::SmolStr;

pub use common::util::is_valid_kebab_case;

/// Parses a raw rule definition into validated options, condition, scope, and message.
pub struct RuleParser;

impl RuleParser {
    /// Validates and converts raw YAML options into typed `IndexMap<SmolStr, PropertyValue>`.
    #[inline]
    pub fn parse_options(
        rule_id: &str,
        raw_options: &Option<IndexMap<String, serde_yaml::Value>>,
    ) -> Result<Option<IndexMap<SmolStr, PropertyValue>>, ConfigError> {
        options::parse_options(rule_id, raw_options)
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
                rule_id: rule_id.to_string(),
                property: "where".to_string(),
                operator: "root".to_string(),
                reason: "'where' block must be a mapping",
            })?;

        if mapping.is_empty() {
            return Err(ConfigError::InvalidOperator {
                rule_id: rule_id.to_string(),
                property: "where".to_string(),
                operator: "root".to_string(),
                reason: "'where' block cannot be empty",
            });
        }

        if mapping.len() > 1 {
            return Err(ConfigError::InvalidOperator {
                rule_id: rule_id.to_string(),
                property: "where".to_string(),
                operator: "root".to_string(),
                reason: "'where' block must contain exactly one combinator ('all', 'any') or a single property condition",
            });
        }

        if let Some(all_val) = mapping.get(serde_yaml::Value::String("all".to_string())) {
            let seq = all_val
                .as_sequence()
                .ok_or_else(|| ConfigError::InvalidOperator {
                    rule_id: rule_id.to_string(),
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
                    rule_id: rule_id.to_string(),
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
                    rule_id: rule_id.to_string(),
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
                rule_id: rule_id.to_string(),
                property: "where".to_string(),
                operator: "combinator".to_string(),
                reason: "Condition sequence cannot be empty",
            });
        }

        let mut conditions = Vec::with_capacity(seq.len());
        let mut distinct_scopes: smallvec::SmallVec<[Scope; 4]> = smallvec::SmallVec::new();

        for item in seq {
            let (prop_cond, scope) = Self::parse_single_condition_value(rule_id, item, options)?;
            if !distinct_scopes.contains(&scope) {
                distinct_scopes.push(scope);
            }
            conditions.push(prop_cond);
        }

        let target_scope = Scope::resolve_target_scope(&distinct_scopes).ok_or_else(|| {
            let (expected, actual, prop) = distinct_scopes
                .iter()
                .enumerate()
                .find_map(|(i, &s1)| {
                    distinct_scopes[i + 1..].iter().find_map(|&s2| {
                        if !s1.can_be_ancestor_of(s2) && !s2.can_be_ancestor_of(s1) {
                            let offending_prop = conditions
                                .iter()
                                .find(|c| c.property.scope() == s2)
                                .map(|c| c.property.field_name().to_string())
                                .unwrap_or_default();
                            Some((s1.as_str(), s2.as_str(), offending_prop))
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| {
                    (
                        distinct_scopes
                            .first()
                            .map(|s| s.as_str())
                            .unwrap_or("file"),
                        distinct_scopes.get(1).map(|s| s.as_str()).unwrap_or("file"),
                        conditions
                            .last()
                            .map(|c| c.property.field_name().to_string())
                            .unwrap_or_default(),
                    )
                });
            ConfigError::ScopeMismatch {
                rule_id: rule_id.to_string(),
                expected_scope: expected,
                actual_scope: actual,
                property: prop,
            }
        })?;

        if target_scope != Scope::File {
            for cond in &conditions {
                if cond.property.is_deferred_file_aggregate() {
                    return Err(ConfigError::DeferredPropertyInChildScope {
                        rule_id: rule_id.to_string(),
                        property: cond.property.field_name(),
                    });
                }
            }
        }

        Ok((conditions, target_scope))
    }

    fn parse_single_condition_value(
        rule_id: &str,
        value: &serde_yaml::Value,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(PropertyCondition, Scope), ConfigError> {
        let mapping = value
            .as_mapping()
            .ok_or_else(|| ConfigError::InvalidOperator {
                rule_id: rule_id.to_string(),
                property: "condition".to_string(),
                operator: "item".to_string(),
                reason: "Condition item must be a mapping of '<scope>.<property>: <op>'",
            })?;

        if mapping.is_empty() {
            return Err(ConfigError::InvalidOperator {
                rule_id: rule_id.to_string(),
                property: "condition".to_string(),
                operator: "item".to_string(),
                reason: "Condition item cannot be empty",
            });
        }

        if mapping.len() > 1 {
            return Err(ConfigError::InvalidOperator {
                rule_id: rule_id.to_string(),
                property: "condition".to_string(),
                operator: "item".to_string(),
                reason: "Condition item must contain exactly one '<scope>.<property>: <op>' pair",
            });
        }

        let (key_val, op_val) = mapping.iter().next().unwrap();

        let key_str = key_val
            .as_str()
            .ok_or_else(|| ConfigError::InvalidOperator {
                rule_id: rule_id.to_string(),
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
                    rule_id: rule_id.to_string(),
                    property: property_path.to_string(),
                    operator: "path".to_string(),
                    reason: "Property must be scoped in '<scope>.<property>' format",
                })?;

        let scope = Scope::parse(scope_str).ok_or_else(|| ConfigError::UnknownScope {
            rule_id: rule_id.to_string(),
            scope: scope_str.to_string(),
            property: property_path.to_string(),
        })?;

        let property_key =
            PropertyKey::from_scope_and_field(scope, field_str).ok_or_else(|| {
                ConfigError::UnknownProperty {
                    rule_id: rule_id.to_string(),
                    scope: scope_str.to_string(),
                    property: field_str.to_string(),
                }
            })?;

        let op = parse_operator(rule_id, property_path, op_val, options)?;
        validate_property_operator(rule_id, property_key, property_path, &op, options)?;
        Ok((PropertyCondition::new(property_key, op), scope))
    }

    /// Validates message template placeholders against participating scopes in a condition tree.
    pub fn validate_message_template_for_condition(
        rule_id: &str,
        condition: &Condition,
        rule_scope: Scope,
        template: &str,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> Result<(), ConfigError> {
        let mut allowed_scopes = condition.participating_scopes();
        if !allowed_scopes.contains(&rule_scope) {
            allowed_scopes.push(rule_scope);
        }
        crate::config::template::validate_message_template(
            rule_id,
            rule_scope,
            &allowed_scopes,
            template,
            options,
        )
    }
}
