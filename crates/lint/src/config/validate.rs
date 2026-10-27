//! Property operator compatibility validator.

use crate::condition::{CondOp, Condition};
use crate::config::error::ConfigError;
use crate::types::{Operand, PropertyKey, PropertyValue, ReportTarget};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

pub fn validate_and_compile_report_targets(
    rule_id: &str,
    raw_targets: Option<&[String]>,
    condition: &Condition,
) -> Result<SmallVec<[ReportTarget; 2]>, ConfigError> {
    let Some(targets) = raw_targets else {
        return Ok(SmallVec::new());
    };

    let participating = condition.participating_scopes();
    let mut compiled = SmallVec::with_capacity(targets.len());

    for raw_target in targets {
        let target =
            ReportTarget::parse(raw_target).ok_or_else(|| ConfigError::UnknownReportTarget {
                rule_id: rule_id.to_string(),
                target: raw_target.clone(),
            })?;

        let target_scope = target.scope();
        if !participating.contains(&target_scope) {
            return Err(ConfigError::InvalidReportTargetScope {
                rule_id: rule_id.to_string(),
                target: raw_target.clone(),
                target_scope: target_scope.as_str(),
            });
        }

        if !compiled.contains(&target) {
            compiled.push(target);
        }
    }

    Ok(compiled)
}

pub fn validate_property_operator(
    rule_id: &str,
    property_key: PropertyKey,
    property_path: &str,
    op: &CondOp,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<(), ConfigError> {
    let prop_type = property_key.expected_type_name();
    match op {
        CondOp::Gt(_) | CondOp::Lt(_) | CondOp::Gte(_) | CondOp::Lte(_) => {
            if prop_type != "Int" && prop_type != "Float" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    rule_id: rule_id.to_string(),
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: match op {
                        CondOp::Gt(_) => "gt",
                        CondOp::Lt(_) => "lt",
                        CondOp::Gte(_) => "gte",
                        _ => "lte",
                    },
                });
            }
        }
        CondOp::Has(_) => {
            if prop_type != "StrList" && prop_type != "Str" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    rule_id: rule_id.to_string(),
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: "has",
                });
            }
        }
        CondOp::StartsWith(_) | CondOp::EndsWith(_) => {
            if prop_type != "Str" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    rule_id: rule_id.to_string(),
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: if matches!(op, CondOp::StartsWith(_)) {
                        "startswith"
                    } else {
                        "endswith"
                    },
                });
            }
        }
        CondOp::Matches { .. } => {
            if prop_type != "Str" {
                return Err(ConfigError::IncompatiblePropertyOperator {
                    rule_id: rule_id.to_string(),
                    property: property_path.to_string(),
                    property_type: prop_type,
                    operator: "matches",
                });
            }
        }
        CondOp::Eq(operand) => {
            validate_eq_operand(rule_id, property_path, prop_type, operand, options)?
        }
        CondOp::Not(inner) => {
            validate_property_operator(rule_id, property_key, property_path, inner, options)?;
        }
    }
    Ok(())
}

fn validate_eq_operand(
    rule_id: &str,
    property_path: &str,
    prop_type: &'static str,
    operand: &Operand<PropertyValue>,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<(), ConfigError> {
    if prop_type == "StrList" {
        return Err(ConfigError::IncompatiblePropertyOperator {
            rule_id: rule_id.to_string(),
            property: property_path.to_string(),
            property_type: prop_type,
            operator: "eq",
        });
    }
    let actual_type = match operand {
        Operand::Literal(literal_value) => literal_value.type_name(),
        Operand::OptionRef(option_key) => options
            .and_then(|options_map| options_map.get(option_key))
            .map(|option_value| option_value.type_name())
            .unwrap_or(prop_type),
    };
    if actual_type != prop_type {
        return Err(ConfigError::InvalidOperandType {
            rule_id: rule_id.to_string(),
            property: property_path.to_string(),
            operator: "eq",
            expected: prop_type,
            actual: actual_type.to_string(),
        });
    }
    Ok(())
}
