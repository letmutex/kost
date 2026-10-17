//! Rule registry managing active AST rules and statistical rules.

use crate::rule::{AstRule, StatRule};
use crate::types::{PropertyKey, Scope, Severity};
use ahash::{AHashMap, AHashSet};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Compiled and indexed registry of active lint rules.
pub struct RuleRegistry {
    pub ast_rules: Vec<Box<dyn AstRule>>,
    /// Precomputed canonical or overridden report IDs (parallel to ast_rules)
    pub ast_report_ids: Vec<SmolStr>,
    /// Node kind string -> indices in `ast_rules` for O(1) dispatch
    pub ast_dispatch: AHashMap<&'static str, SmallVec<[usize; 4]>>,
    /// Catch-all AST rules that inspect every node kind
    pub ast_catch_all: SmallVec<[usize; 2]>,
    /// Configured severities for AST rules (parallel to ast_rules)
    pub ast_severities: Vec<Severity>,
    /// Target scope -> active statistical rules
    pub stat_dispatch: AHashMap<Scope, Vec<StatRule>>,
    /// Set of property keys needed by at least one active statistical rule
    pub needed_properties: AHashSet<PropertyKey>,
    /// Whether any active AST rule requires file-level exclusion ranges
    pub needs_file_exclusions: bool,
}

impl RuleRegistry {
    pub fn builder() -> RuleRegistryBuilder {
        RuleRegistryBuilder::new()
    }
}

/// Builder for assembling and compiling the `RuleRegistry`.
pub struct RuleRegistryBuilder {
    ast_rules: Vec<Box<dyn AstRule>>,
    stat_rules: Vec<StatRule>,
    severity_overrides: AHashMap<SmolStr, Severity>,
    report_id_overrides: AHashMap<SmolStr, SmolStr>,
}

impl Default for RuleRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleRegistryBuilder {
    pub fn new() -> Self {
        Self {
            ast_rules: Vec::new(),
            stat_rules: Vec::new(),
            severity_overrides: AHashMap::new(),
            report_id_overrides: AHashMap::new(),
        }
    }

    pub fn register_ast<R: AstRule + 'static>(&mut self, rule: R) -> &mut Self {
        self.register_ast_boxed(Box::new(rule))
    }

    pub fn register_ast_boxed(&mut self, rule: Box<dyn AstRule>) -> &mut Self {
        self.ast_rules.push(rule);
        self
    }

    pub fn register_stat(&mut self, rule: StatRule) -> &mut Self {
        self.stat_rules.push(rule);
        self
    }

    pub fn set_severity(&mut self, rule_id: impl Into<SmolStr>, severity: Severity) -> &mut Self {
        self.severity_overrides.insert(rule_id.into(), severity);
        self
    }

    pub fn set_report_id(
        &mut self,
        rule_id: impl Into<SmolStr>,
        report_id: impl Into<SmolStr>,
    ) -> &mut Self {
        self.report_id_overrides
            .insert(rule_id.into(), report_id.into());
        self
    }

    pub fn build(self) -> RuleRegistry {
        let mut active_ast_rules = Vec::new();
        let mut ast_report_ids = Vec::new();
        let mut ast_severities = Vec::new();
        let mut ast_dispatch: AHashMap<&'static str, SmallVec<[usize; 4]>> = AHashMap::new();
        let mut ast_catch_all = SmallVec::new();
        let mut needs_file_exclusions = false;

        for rule in self.ast_rules {
            let full_id = crate::concat_smol_str!("standard:", rule.id());
            let severity = self
                .severity_overrides
                .get(&full_id)
                .copied()
                .unwrap_or(Severity::Warning);

            if severity == Severity::None {
                continue;
            }

            if rule.needs_file_exclusions() {
                needs_file_exclusions = true;
            }

            let report_id = self
                .report_id_overrides
                .get(&full_id)
                .cloned()
                .unwrap_or_else(|| full_id.clone());

            let index = active_ast_rules.len();
            let kinds = rule.interested_kinds();
            if kinds.is_empty() {
                ast_catch_all.push(index);
            } else {
                for &kind in kinds {
                    ast_dispatch.entry(kind).or_default().push(index);
                }
            }

            active_ast_rules.push(rule);
            ast_report_ids.push(report_id);
            ast_severities.push(severity);
        }

        let (stat_dispatch, needed_properties) = compile_stat_rules(
            self.stat_rules,
            &self.severity_overrides,
            &self.report_id_overrides,
        );

        RuleRegistry {
            ast_rules: active_ast_rules,
            ast_report_ids,
            ast_dispatch,
            ast_catch_all,
            ast_severities,
            stat_dispatch,
            needed_properties,
            needs_file_exclusions,
        }
    }
}

fn compile_stat_rules(
    stat_rules: Vec<StatRule>,
    severity_overrides: &AHashMap<SmolStr, Severity>,
    report_id_overrides: &AHashMap<SmolStr, SmolStr>,
) -> (AHashMap<Scope, Vec<StatRule>>, AHashSet<PropertyKey>) {
    let mut stat_dispatch: AHashMap<Scope, Vec<StatRule>> = AHashMap::new();
    let mut needed_properties = AHashSet::new();

    for mut rule in stat_rules {
        if let Some(severity) = severity_overrides.get(&rule.id) {
            rule.severity = *severity;
        }

        if let Some(report_id) = report_id_overrides.get(&rule.id) {
            rule.report_id = report_id.clone();
        }

        if rule.severity == Severity::None {
            continue;
        }

        for prop in rule.referenced_properties() {
            needed_properties.insert(prop);
        }

        stat_dispatch.entry(rule.scope).or_default().push(rule);
    }

    (stat_dispatch, needed_properties)
}

impl std::fmt::Debug for RuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleRegistry")
            .field("ast_rules_count", &self.ast_rules.len())
            .field("ast_report_ids", &self.ast_report_ids)
            .field("ast_severities", &self.ast_severities)
            .field("stat_dispatch", &self.stat_dispatch)
            .field("needed_properties", &self.needed_properties)
            .field("needs_file_exclusions", &self.needs_file_exclusions)
            .finish()
    }
}
