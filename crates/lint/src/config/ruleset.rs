//! Exported ruleset and rule lookup models.

use crate::registry::{RuleRegistry, RuleRegistryBuilder};
use crate::rule::StatRule;
use crate::types::Severity;
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
