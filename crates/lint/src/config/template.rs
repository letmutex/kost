//! Message template placeholder validation for statistical rules.

use crate::config::error::ConfigError;
use crate::types::{PropertyKey, PropertyValue, Scope};
use indexmap::IndexMap;
use smol_str::SmolStr;

/// Strictly validates message template placeholders (`${scope.property}` and `${options.key}`).
pub fn validate_message_template(
    target_scope: Scope,
    allowed_scopes: &[Scope],
    template: &str,
    options: Option<&IndexMap<SmolStr, PropertyValue>>,
) -> Result<(), ConfigError> {
    let mut cursor = 0;
    while let Some(start_index) = template[cursor..].find("${") {
        let actual_start = cursor + start_index;
        if let Some(end_index) = template[actual_start..].find('}') {
            let actual_end = actual_start + end_index;
            let placeholder = &template[actual_start + 2..actual_end];

            if let Some(opt_key) = placeholder.strip_prefix("options.") {
                let has_option = options.is_some_and(|m| m.contains_key(opt_key));
                if !has_option {
                    return Err(ConfigError::InvalidPlaceholder {
                        placeholder: placeholder.to_string(),
                        reason: "Option reference does not exist in declared rule options",
                    });
                }
            } else if let Some((scope_prefix, field_name)) = placeholder.split_once('.') {
                let scope =
                    Scope::parse(scope_prefix).ok_or_else(|| ConfigError::InvalidPlaceholder {
                        placeholder: placeholder.to_string(),
                        reason: "Unknown scope prefix in placeholder",
                    })?;

                if !allowed_scopes.contains(&scope) {
                    return Err(ConfigError::InvalidPlaceholder {
                        placeholder: placeholder.to_string(),
                        reason: "Scope prefix does not match any participating scope in rule or conditions",
                    });
                }

                let property_key = PropertyKey::from_scope_and_field(scope, field_name)
                    .ok_or_else(|| ConfigError::InvalidPlaceholder {
                        placeholder: placeholder.to_string(),
                        reason: "Unknown property for scope in placeholder",
                    })?;

                if target_scope != Scope::File && property_key.is_deferred_file_aggregate() {
                    return Err(ConfigError::InvalidPlaceholder {
                        placeholder: placeholder.to_string(),
                        reason: "Deferred file aggregate property cannot be referenced in child-scoped rules",
                    });
                }
            } else {
                return Err(ConfigError::InvalidPlaceholder {
                    placeholder: placeholder.to_string(),
                    reason: "Placeholder must be '${options.<key>}' or '${<scope>.<property>}'",
                });
            }

            cursor = actual_end + 1;
        } else {
            let unclosed = &template[actual_start + 2..];
            return Err(ConfigError::InvalidPlaceholder {
                placeholder: unclosed.to_string(),
                reason: "Unclosed placeholder, missing '}'",
            });
        }
    }

    Ok(())
}
