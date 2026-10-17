//! Rule merging and override evaluation.

use crate::config::error::ConfigError;
use crate::config::parser::{RuleParser, is_valid_kebab_case};
use crate::config::schema::RawRule;
use crate::registry::{RuleRegistry, RuleRegistryBuilder};
use crate::rule::StatRule;
use crate::types::{Severity, WherePolicy};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// In-memory representation of an active rule during configuration loading.
#[derive(Debug, Clone)]
pub enum MergedRule {
    Ast {
        id: SmolStr,
        severity: Severity,
        report_id: SmolStr,
    },
    Stat(Box<StatRule>),
}

/// Result of looking up an included rule by identifier.
#[derive(Debug)]
pub enum RuleLookup<'a> {
    Found(&'a SmolStr, &'a MergedRule),
    Ambiguous(Vec<String>),
    NotFound,
}

/// Ruleset exported by an evaluated configuration file or virtual ruleset.
#[derive(Clone, Default)]
pub struct ExportedRuleset {
    pub rules: IndexMap<SmolStr, MergedRule>,
}

impl ExportedRuleset {
    pub fn find_rule(&self, requested: &str) -> RuleLookup<'_> {
        if let Some((id, rule)) = self.rules.get_key_value(requested) {
            return RuleLookup::Found(id, rule);
        }
        let mut matches: SmallVec<[(&SmolStr, &MergedRule); 2]> = SmallVec::new();
        for (id, rule) in &self.rules {
            if let Some((_scope, name)) = id.split_once(':')
                && name == requested
            {
                matches.push((id, rule));
            }
        }
        match matches.len() {
            0 => RuleLookup::NotFound,
            1 => RuleLookup::Found(matches[0].0, matches[0].1),
            _ => {
                let candidates = matches.into_iter().map(|(id, _)| id.to_string()).collect();
                RuleLookup::Ambiguous(candidates)
            }
        }
    }

    pub fn into_registry(self) -> RuleRegistry {
        let mut builder = RuleRegistryBuilder::new();
        for (id, rule) in self.rules {
            match rule {
                MergedRule::Ast {
                    severity,
                    report_id,
                    ..
                } => {
                    if severity == Severity::None {
                        continue;
                    }
                    let base_name = id.strip_prefix("standard:").unwrap_or(&id);
                    if let Some(ast_rule) =
                        crate::rules::standard::create_standard_ast_rule(base_name)
                    {
                        builder.register_ast_boxed(ast_rule);
                        builder.set_severity(id.clone(), severity);
                        builder.set_report_id(id, report_id);
                    }
                }
                MergedRule::Stat(stat_rule) => {
                    if stat_rule.severity != Severity::None {
                        builder.register_stat(*stat_rule);
                    }
                }
            }
        }
        builder.build()
    }
}

/// Stores and merges rules across included configuration files.
#[derive(Default)]
pub struct RuleMerger {
    pub rules_map: IndexMap<SmolStr, MergedRule>,
}

impl RuleMerger {
    pub fn new() -> Self {
        Self {
            rules_map: IndexMap::new(),
        }
    }

    pub fn insert_merged(&mut self, full_id: SmolStr, rule: MergedRule) {
        self.rules_map.insert(full_id, rule);
    }

    pub fn into_exported(self) -> ExportedRuleset {
        ExportedRuleset {
            rules: self.rules_map,
        }
    }

    pub fn insert_ast(&mut self, full_id: SmolStr, severity: Severity) {
        self.rules_map
            .entry(full_id.clone())
            .or_insert_with(|| MergedRule::Ast {
                id: full_id.clone(),
                severity,
                report_id: full_id,
            });
    }

    pub fn insert_stat(&mut self, rule: StatRule) {
        self.rules_map
            .insert(rule.id.clone(), MergedRule::Stat(Box::new(rule)));
    }

    pub fn process_entry(&mut self, raw: RawRule, current_file: &str) -> Result<(), ConfigError> {
        let has_colon = raw.id.contains(':');
        if has_colon {
            self.process_override(raw, current_file)
        } else {
            self.process_custom(raw, current_file)
        }
    }

    fn process_override(&mut self, raw: RawRule, current_file: &str) -> Result<(), ConfigError> {
        if raw.allow_where_override.is_some() {
            return Err(ConfigError::DisallowedFieldOnOverride {
                rule_id: raw.id,
                field: "allow_where_override",
            });
        }

        let id_string = raw.id.clone();
        let (_target_scope, base_name) = match id_string.split_once(':') {
            Some((scope, name)) if !scope.is_empty() && !name.is_empty() => (scope, name),
            _ => {
                return Err(ConfigError::InvalidIdFormat {
                    id: raw.id,
                    reason: "Overriding rule ID must be in '<scope>:<name>' format",
                });
            }
        };

        let full_id = SmolStr::new(&raw.id);
        let target_rule =
            self.rules_map
                .get_mut(&full_id)
                .ok_or_else(|| ConfigError::UnknownRuleOverride {
                    id: raw.id.clone(),
                    file: current_file.to_string(),
                })?;

        let new_report_id = crate::concat_smol_str!(current_file, ":", base_name);

        match target_rule {
            MergedRule::Ast {
                severity,
                report_id,
                ..
            } => {
                if raw.where_clause.is_some() || raw.options.is_some() || raw.message.is_some() {
                    return Err(ConfigError::InvalidAstOverride { id: raw.id });
                }
                if let Some(sev_str) = raw.severity.as_deref() {
                    let sev =
                        Severity::parse(sev_str).ok_or_else(|| ConfigError::InvalidSeverity {
                            rule_id: raw.id.to_string(),
                            severity: sev_str.to_string(),
                        })?;
                    *severity = sev;
                }
                *report_id = new_report_id;
            }
            MergedRule::Stat(stat) => {
                Self::override_stat_rule(stat, raw, &full_id, new_report_id)?;
            }
        }

        Ok(())
    }

    fn override_stat_rule(
        stat: &mut StatRule,
        raw: RawRule,
        full_id: &SmolStr,
        new_report_id: SmolStr,
    ) -> Result<(), ConfigError> {
        if let Some(sev_str) = raw.severity.as_deref() {
            let sev = Severity::parse(sev_str).ok_or_else(|| ConfigError::InvalidSeverity {
                rule_id: full_id.to_string(),
                severity: sev_str.to_string(),
            })?;
            stat.severity = sev;
        }

        if let Some(raw_opts) = raw.options {
            Self::override_options(stat, full_id, raw_opts)?;
        }

        if let Some(raw_where) = raw.where_clause {
            if !stat.where_policy.is_allowed() {
                return Err(ConfigError::WhereOverrideRejected {
                    id: full_id.to_string(),
                });
            }
            let (new_condition, inferred_scope) =
                RuleParser::parse_where(full_id, &raw_where, stat.options.as_ref())?;
            if inferred_scope != stat.scope {
                return Err(ConfigError::ScopeMismatch {
                    rule_id: full_id.to_string(),
                    expected_scope: stat.scope.as_str(),
                    actual_scope: inferred_scope.as_str(),
                    property: "where".to_string(),
                });
            }
            stat.condition = new_condition;
        }

        if let Some(new_msg) = raw.message {
            RuleParser::validate_message_template(stat.scope, &new_msg, stat.options.as_ref())?;
            stat.message_template = SmolStr::new(new_msg);
        }

        stat.report_id = new_report_id;
        Ok(())
    }

    fn override_options(
        stat: &mut StatRule,
        full_id: &SmolStr,
        raw_opts: IndexMap<String, serde_yaml::Value>,
    ) -> Result<(), ConfigError> {
        if raw_opts.is_empty() {
            return Err(ConfigError::EmptyOptions {
                rule_id: full_id.to_string(),
            });
        }
        let existing_options = stat.options.get_or_insert_with(IndexMap::new);
        for (key, val) in raw_opts {
            let key_smol = SmolStr::new(&key);
            let existing_val =
                existing_options
                    .get(&key_smol)
                    .ok_or_else(|| ConfigError::OptionNotDeclared {
                        rule_id: full_id.to_string(),
                        option_key: key.clone(),
                    })?;

            let parsed_val =
                RuleParser::parse_options(full_id, &Some(IndexMap::from([(key.clone(), val)])))?
                    .and_then(|mut m| m.swap_remove(&key_smol))
                    .unwrap();

            if existing_val.type_name() != parsed_val.type_name() {
                return Err(ConfigError::OptionTypeMismatch {
                    rule_id: full_id.to_string(),
                    option_key: key,
                    expected_type: existing_val.type_name(),
                    actual_type: parsed_val.type_name(),
                });
            }

            existing_options.insert(key_smol, parsed_val);
        }
        stat.condition.update_options(existing_options)?;
        Ok(())
    }

    fn process_custom(&mut self, raw: RawRule, current_file: &str) -> Result<(), ConfigError> {
        let id_str = raw.id.trim();
        if !is_valid_kebab_case(id_str) {
            return Err(ConfigError::InvalidIdFormat {
                id: id_str.to_string(),
                reason: "Custom rule ID must be kebab-case and cannot contain ':'",
            });
        }

        let full_id = crate::concat_smol_str!(current_file, ":", id_str);
        let options = RuleParser::parse_options(&full_id, &raw.options)?;

        let raw_where = raw
            .where_clause
            .ok_or_else(|| ConfigError::MissingRequiredField {
                rule_id: full_id.to_string(),
                field: "where",
            })?;

        let (condition, rule_scope) =
            RuleParser::parse_where(&full_id, &raw_where, options.as_ref())?;

        let message_str = raw
            .message
            .ok_or_else(|| ConfigError::MissingRequiredField {
                rule_id: full_id.to_string(),
                field: "message",
            })?;

        RuleParser::validate_message_template(rule_scope, &message_str, options.as_ref())?;

        let severity = match raw.severity.as_deref() {
            Some(sev_str) => {
                Severity::parse(sev_str).ok_or_else(|| ConfigError::InvalidSeverity {
                    rule_id: full_id.to_string(),
                    severity: sev_str.to_string(),
                })?
            }
            None => Severity::Warning,
        };

        let policy = WherePolicy::from_allow_bool(raw.allow_where_override.unwrap_or(true));

        let stat_rule = StatRule {
            id: full_id.clone(),
            report_id: full_id.clone(),
            severity,
            scope: rule_scope,
            options,
            condition,
            message_template: SmolStr::new(message_str),
            where_policy: policy,
        };

        self.rules_map
            .insert(full_id, MergedRule::Stat(Box::new(stat_rule)));
        Ok(())
    }
}
