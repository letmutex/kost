//! Parser for condition operators and operands.

use crate::condition::CondOp;
use crate::config::error::ConfigError;
use crate::types::{Numeric, Operand, PropertyKey, PropertyValue};
use indexmap::IndexMap;
use smol_str::SmolStr;

/// Parses an operator expression string (e.g. `gt(60)`, `has("@Composable")`, `not(...)`).
pub fn parse_operator(
    rule_id: &str,
    property_path: &str,
    op_val: &serde_yaml::Value,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<CondOp, ConfigError> {
    let op_str = match op_val {
        serde_yaml::Value::String(s) => s.trim(),
        _ => {
            return Err(ConfigError::InvalidOperator {
                property: property_path.to_string(),
                operator: format!("{:?}", op_val),
                reason: "Operator expression must be a string (e.g. gt(60), has('@Marker'))",
            });
        }
    };

    if let Some((op_name, rest)) = op_str.split_once('(')
        && let Some(arg) = rest.strip_suffix(')')
    {
        let trimmed_arg = arg.trim();
        if let Some(res) =
            parse_comparison_op(rule_id, property_path, op_name, trimmed_arg, options)
        {
            return res;
        }

        match op_name {
            "not" => {
                let inner_val = serde_yaml::Value::String(trimmed_arg.to_string());
                let inner_op = parse_operator(rule_id, property_path, &inner_val, options)?;
                return Ok(CondOp::Not(Box::new(inner_op)));
            }
            "eq" => {
                let operand = parse_value_operand(rule_id, property_path, trimmed_arg, options)?;
                return Ok(CondOp::Eq(operand));
            }
            "has" => {
                let operand =
                    parse_str_operand(rule_id, property_path, "has", trimmed_arg, options)?;
                return Ok(CondOp::Has(operand));
            }
            "matches" => {
                let (pattern_str, option_ref) =
                    parse_regex_pattern(rule_id, property_path, trimmed_arg, options)?;
                let regex =
                    regex::Regex::new(pattern_str).map_err(|_| ConfigError::InvalidOperator {
                        property: property_path.to_string(),
                        operator: "matches".to_string(),
                        reason: "Invalid regular expression pattern",
                    })?;
                return Ok(CondOp::Matches { regex, option_ref });
            }

            _ => {}
        }
    }

    Err(ConfigError::InvalidOperator {
        property: property_path.to_string(),
        operator: op_str.to_string(),
        reason: "Unknown operator; expected one of gt, lt, gte, lte, eq, has, matches, not",
    })
}

fn parse_comparison_op(
    rule_id: &str,
    property_path: &str,
    op_name: &str,
    arg: &str,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Option<Result<CondOp, ConfigError>> {
    match op_name {
        "gt" => {
            Some(parse_numeric_operand(rule_id, property_path, "gt", arg, options).map(CondOp::Gt))
        }
        "lt" => {
            Some(parse_numeric_operand(rule_id, property_path, "lt", arg, options).map(CondOp::Lt))
        }
        "gte" => Some(
            parse_numeric_operand(rule_id, property_path, "gte", arg, options).map(CondOp::Gte),
        ),
        "lte" => Some(
            parse_numeric_operand(rule_id, property_path, "lte", arg, options).map(CondOp::Lte),
        ),
        _ => None,
    }
}

fn parse_numeric_operand(
    rule_id: &str,
    property_path: &str,
    operator: &'static str,
    arg: &str,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<Operand<Numeric>, ConfigError> {
    if let Some(opt_key) = arg.strip_prefix("options.") {
        let opt_smol = SmolStr::new(opt_key);
        let options_map = options.ok_or_else(|| ConfigError::OptionNotDeclared {
            rule_id: rule_id.to_string(),
            option_key: opt_key.to_string(),
        })?;
        if let Some(val) = options_map.get(&opt_smol) {
            if val.as_numeric().is_none() {
                return Err(ConfigError::InvalidOperandType {
                    property: property_path.to_string(),
                    operator,
                    expected: "Int or Float option",
                    actual: val.type_name().to_string(),
                });
            }
        } else {
            return Err(ConfigError::OptionNotDeclared {
                rule_id: rule_id.to_string(),
                option_key: opt_key.to_string(),
            });
        }
        return Ok(Operand::OptionRef(opt_smol));
    }

    if let Ok(int_val) = arg.parse::<i64>() {
        return Ok(Operand::Literal(Numeric::Int(int_val)));
    }

    if let Ok(float_val) = arg.parse::<f64>() {
        return Ok(Operand::Literal(Numeric::Float(float_val)));
    }

    Err(ConfigError::InvalidOperandType {
        property: property_path.to_string(),
        operator,
        expected: "numeric literal or numeric options reference",
        actual: arg.to_string(),
    })
}

fn parse_str_operand(
    rule_id: &str,
    property_path: &str,
    operator: &'static str,
    arg: &str,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<Operand<SmolStr>, ConfigError> {
    if let Some(opt_key) = arg.strip_prefix("options.") {
        let opt_smol = SmolStr::new(opt_key);
        let options_map = options.ok_or_else(|| ConfigError::OptionNotDeclared {
            rule_id: rule_id.to_string(),
            option_key: opt_key.to_string(),
        })?;
        if let Some(val) = options_map.get(&opt_smol) {
            if !matches!(val, PropertyValue::Str(_)) {
                return Err(ConfigError::InvalidOperandType {
                    property: property_path.to_string(),
                    operator,
                    expected: "Str option",
                    actual: val.type_name().to_string(),
                });
            }
        } else {
            return Err(ConfigError::OptionNotDeclared {
                rule_id: rule_id.to_string(),
                option_key: opt_key.to_string(),
            });
        }
        return Ok(Operand::OptionRef(opt_smol));
    }

    let clean_str = unquote(arg);
    Ok(Operand::Literal(SmolStr::new(clean_str)))
}

fn parse_value_operand(
    rule_id: &str,
    _property_path: &str,
    arg: &str,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<Operand<PropertyValue>, ConfigError> {
    if let Some(opt_key) = arg.strip_prefix("options.") {
        let opt_smol = SmolStr::new(opt_key);
        let options_map = options.ok_or_else(|| ConfigError::OptionNotDeclared {
            rule_id: rule_id.to_string(),
            option_key: opt_key.to_string(),
        })?;
        if !options_map.contains_key(&opt_smol) {
            return Err(ConfigError::OptionNotDeclared {
                rule_id: rule_id.to_string(),
                option_key: opt_key.to_string(),
            });
        }
        return Ok(Operand::OptionRef(opt_smol));
    }

    if let Ok(b) = arg.parse::<bool>() {
        return Ok(Operand::Literal(PropertyValue::Bool(b)));
    }
    if let Ok(n) = arg.parse::<i64>() {
        return Ok(Operand::Literal(PropertyValue::Int(n)));
    }
    if let Ok(f) = arg.parse::<f64>() {
        return Ok(Operand::Literal(PropertyValue::Float(f)));
    }

    let clean = unquote(arg);
    Ok(Operand::Literal(PropertyValue::Str(SmolStr::new(clean))))
}

/// Trims leading and trailing matching quotes from string literals.
pub fn unquote(s: &str) -> &str {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        if trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        }
    } else {
        trimmed
    }
}

fn parse_regex_pattern<'a>(
    rule_id: &str,
    property_path: &str,
    arg: &'a str,
    options: Option<&'a IndexMap<SmolStr, PropertyValue>>,
) -> Result<(&'a str, Option<SmolStr>), ConfigError> {
    if let Some(opt_key) = arg.strip_prefix("options.") {
        let opt_smol = SmolStr::new(opt_key);
        let options_map = options.ok_or_else(|| ConfigError::OptionNotDeclared {
            rule_id: rule_id.to_string(),
            option_key: opt_key.to_string(),
        })?;
        match options_map.get(&opt_smol) {
            Some(PropertyValue::Str(s)) => Ok((s.as_str(), Some(opt_smol))),
            Some(val) => Err(ConfigError::InvalidOperandType {
                property: property_path.to_string(),
                operator: "matches",
                expected: "Str option",
                actual: val.type_name().to_string(),
            }),
            None => Err(ConfigError::OptionNotDeclared {
                rule_id: rule_id.to_string(),
                option_key: opt_key.to_string(),
            }),
        }
    } else {
        Ok((unquote(arg), None))
    }
}

pub fn validate_property_operator(
    property_key: PropertyKey,
    property_path: &str,
    op: &CondOp,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<(), ConfigError> {
    let prop_type = property_key.expected_type_name();
    match op {
        CondOp::Gt(_) | CondOp::Lt(_) | CondOp::Gte(_) | CondOp::Lte(_) => {
            if prop_type != "Int" && prop_type != "Float" {
                let op_str = match op {
                    CondOp::Gt(_) => "gt",
                    CondOp::Lt(_) => "lt",
                    CondOp::Gte(_) => "gte",
                    CondOp::Lte(_) => "lte",
                    _ => unreachable!(),
                };
                return Err(ConfigError::IncompatiblePropertyOperator {
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: op_str,
                });
            }
        }
        CondOp::Has(_) => {
            if prop_type != "StrList" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: "has",
                });
            }
        }
        CondOp::Matches { .. } => {
            if prop_type != "Str" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: "matches",
                });
            }
        }
        CondOp::Eq(operand) => {
            if prop_type == "StrList" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: "eq",
                });
            }
            let actual_type = match operand {
                Operand::Literal(v) => v.type_name(),
                Operand::OptionRef(opt) => options
                    .and_then(|m| m.get(opt))
                    .map(|v| v.type_name())
                    .unwrap_or(prop_type),
            };
            if actual_type != prop_type {
                return Err(ConfigError::InvalidOperandType {
                    property: property_path.to_string(),
                    operator: "eq",
                    expected: prop_type,
                    actual: actual_type.to_string(),
                });
            }
        }
        CondOp::Not(inner) => {
            validate_property_operator(property_key, property_path, inner, options)?;
        }
    }
    Ok(())
}
