//! Declarative condition tree evaluation for statistical rules.

use crate::config::error::ConfigError;
use crate::types::{Numeric, Operand, PropertyKey, PropertyValue, Scope};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Trait for resolving property values from a scope or scope stack.
pub trait PropertyLookup {
    fn get_property(&self, key: PropertyKey) -> Option<&PropertyValue>;
}

/// A property bag containing accumulated properties for a lexical scope.
#[derive(Debug, Clone, Default)]
pub struct PropertyBag {
    pub entries: SmallVec<[(PropertyKey, PropertyValue); 8]>,
}

impl PropertyLookup for PropertyBag {
    #[inline]
    fn get_property(&self, key: PropertyKey) -> Option<&PropertyValue> {
        self.get(key)
    }
}

impl PropertyBag {
    pub fn new() -> Self {
        Self {
            entries: SmallVec::new(),
        }
    }

    pub fn insert(&mut self, key: PropertyKey, value: PropertyValue) {
        if let Some(existing) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    pub fn get(&self, key: PropertyKey) -> Option<&PropertyValue> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| value)
    }
}

/// Boolean condition tree over scope properties.
#[derive(Debug, Clone)]
pub enum Condition {
    All(Vec<PropertyCondition>),
    Any(Vec<PropertyCondition>),
    Single(PropertyCondition),
}

impl Condition {
    pub fn matches<P: PropertyLookup + ?Sized>(
        &self,
        properties: &P,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> bool {
        match self {
            Self::All(conditions) => conditions
                .iter()
                .all(|condition| condition.eval(properties, options)),
            Self::Any(conditions) => conditions
                .iter()
                .any(|condition| condition.eval(properties, options)),
            Self::Single(condition) => condition.eval(properties, options),
        }
    }

    pub fn participating_scopes(&self) -> SmallVec<[Scope; 4]> {
        let mut scopes = SmallVec::new();
        for key in self.referenced_properties() {
            let scope = key.scope();
            if !scopes.contains(&scope) {
                scopes.push(scope);
            }
        }
        scopes
    }

    pub fn referenced_properties(&self) -> Vec<PropertyKey> {
        let mut keys = Vec::new();
        match self {
            Self::All(conditions) | Self::Any(conditions) => {
                for condition in conditions {
                    keys.push(condition.property);
                }
            }
            Self::Single(condition) => {
                keys.push(condition.property);
            }
        }
        keys
    }

    pub fn update_options(
        &mut self,
        options: &IndexMap<SmolStr, PropertyValue>,
    ) -> Result<(), ConfigError> {
        match self {
            Self::All(conditions) | Self::Any(conditions) => {
                for condition in conditions {
                    condition.op.update_options(options)?;
                }
            }
            Self::Single(condition) => {
                condition.op.update_options(options)?;
            }
        }
        Ok(())
    }
}

/// A condition applied to a specific property key.
#[derive(Debug, Clone)]
pub struct PropertyCondition {
    pub property: PropertyKey,
    pub op: CondOp,
}

impl PropertyCondition {
    pub fn new(property: PropertyKey, op: CondOp) -> Self {
        Self { property, op }
    }

    pub fn eval<P: PropertyLookup + ?Sized>(
        &self,
        properties: &P,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> bool {
        match properties.get_property(self.property) {
            Some(value) => self.op.eval(value, options),
            None => false,
        }
    }
}

/// Condition operators supported in statistical rule YAML where clauses.
#[derive(Debug, Clone)]
pub enum CondOp {
    Gt(Operand<Numeric>),
    Lt(Operand<Numeric>),
    Gte(Operand<Numeric>),
    Lte(Operand<Numeric>),
    Eq(Operand<PropertyValue>),
    Has(Operand<SmolStr>),
    StartsWith(Operand<SmolStr>),
    EndsWith(Operand<SmolStr>),
    Not(Box<CondOp>),
    Matches {
        regex: regex::Regex,
        option_ref: Option<SmolStr>,
    },
}

impl CondOp {
    pub fn update_options(
        &mut self,
        options: &IndexMap<SmolStr, PropertyValue>,
    ) -> Result<(), ConfigError> {
        match self {
            Self::Matches {
                regex,
                option_ref: Some(opt_key),
            } => {
                if let Some(PropertyValue::Str(new_pattern)) = options.get(opt_key) {
                    let new_regex = regex::Regex::new(new_pattern.as_str()).map_err(|_| {
                        ConfigError::InvalidOperator {
                            property: opt_key.to_string(),
                            operator: "matches".to_string(),
                            reason: "Invalid regular expression pattern in option override",
                        }
                    })?;
                    *regex = new_regex;
                }
            }
            Self::Not(inner) => inner.update_options(options)?,
            _ => {}
        }
        Ok(())
    }

    pub fn eval(
        &self,
        value: &PropertyValue,
        options: Option<&IndexMap<SmolStr, PropertyValue>>,
    ) -> bool {
        match self {
            Self::Gt(operand) => match (value.as_numeric(), resolve_numeric(operand, options)) {
                (Some(val_num), Some(op_num)) => val_num.as_f64() > op_num.as_f64(),
                _ => false,
            },
            Self::Lt(operand) => match (value.as_numeric(), resolve_numeric(operand, options)) {
                (Some(val_num), Some(op_num)) => val_num.as_f64() < op_num.as_f64(),
                _ => false,
            },
            Self::Gte(operand) => match (value.as_numeric(), resolve_numeric(operand, options)) {
                (Some(val_num), Some(op_num)) => val_num.as_f64() >= op_num.as_f64(),
                _ => false,
            },
            Self::Lte(operand) => match (value.as_numeric(), resolve_numeric(operand, options)) {
                (Some(val_num), Some(op_num)) => val_num.as_f64() <= op_num.as_f64(),
                _ => false,
            },
            Self::Eq(operand) => match resolve_value(operand, options) {
                Some(resolved) => value == &resolved,
                None => false,
            },
            Self::Has(operand) => match (value, resolve_str(operand, options)) {
                (PropertyValue::StrList(list), Some(target_str)) => {
                    list.iter().any(|item| item.as_str() == target_str)
                }
                (PropertyValue::Str(string_value), Some(target_str)) => {
                    string_value.contains(target_str)
                }
                _ => false,
            },
            Self::StartsWith(operand) => match (value, resolve_str(operand, options)) {
                (PropertyValue::Str(string_value), Some(target_str)) => {
                    string_value.starts_with(target_str)
                }
                _ => false,
            },
            Self::EndsWith(operand) => match (value, resolve_str(operand, options)) {
                (PropertyValue::Str(string_value), Some(target_str)) => {
                    string_value.ends_with(target_str)
                }
                _ => false,
            },
            Self::Matches { regex, .. } => match value {
                PropertyValue::Str(string_value) => regex.is_match(string_value.as_str()),
                _ => false,
            },
            Self::Not(inner_operator) => !inner_operator.eval(value, options),
        }
    }
}

fn resolve_numeric(
    operand: &Operand<Numeric>,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Option<Numeric> {
    match operand {
        Operand::Literal(numeric_value) => Some(*numeric_value),
        Operand::OptionRef(option_key) => options
            .and_then(|options_map| options_map.get(option_key))
            .and_then(|property_value| property_value.as_numeric()),
    }
}

fn resolve_value(
    operand: &Operand<PropertyValue>,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Option<PropertyValue> {
    match operand {
        Operand::Literal(property_value) => Some(property_value.clone()),
        Operand::OptionRef(option_key) => options
            .and_then(|options_map| options_map.get(option_key))
            .cloned(),
    }
}

fn resolve_str<'a>(
    operand: &'a Operand<SmolStr>,
    options: Option<&'a IndexMap<SmolStr, PropertyValue>>,
) -> Option<&'a str> {
    match operand {
        Operand::Literal(string_value) => Some(string_value.as_str()),
        Operand::OptionRef(option_key) => options
            .and_then(|options_map| options_map.get(option_key))
            .and_then(|property_value| match property_value {
                PropertyValue::Str(string_val) => Some(string_val.as_str()),
                _ => None,
            }),
    }
}
