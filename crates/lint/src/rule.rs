//! Rule definitions for AST rules and statistical rules.

use crate::condition::{Condition, PropertyBag};
use crate::context::LintCtx;
use crate::types::{OptionValue, PropertyKey, Scope, Severity, WherePolicy};
use indexmap::IndexMap;
use smol_str::SmolStr;

/// Imperative compiled rule inspecting specific tree-sitter AST nodes.
pub trait AstRule: Send + Sync {
    /// Canonical local identifier, e.g. "no-semi". The scope "standard:" is attached by the registry.
    fn id(&self) -> &'static str;

    /// Specific tree-sitter node kinds this rule inspects.
    /// Empty slice implies inspection of all nodes.
    fn interested_kinds(&self) -> &'static [&'static str];

    /// Pre-order visit hook: invoked before traversing child nodes.
    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx);

    /// Post-order visit hook: invoked after all child nodes have been traversed.
    fn after_node(&self, _node: tree_sitter::Node, _source: &[u8], _ctx: &mut LintCtx) {}

    /// Whether this rule ignores Ktlint suppressions (e.g. no-unused-imports).
    fn ignores_suppressions(&self) -> bool {
        false
    }

    /// Whether this rule requires file-level string/comment exclusion ranges (e.g. for line length or indent).
    fn needs_file_exclusions(&self) -> bool {
        false
    }
}

/// Declarative statistical rule evaluating condition trees on closed scopes.
#[derive(Debug, Clone)]
pub struct StatRule {
    pub id: SmolStr,
    pub report_id: SmolStr,
    pub severity: Severity,
    pub scope: Scope,
    pub options: Option<IndexMap<SmolStr, OptionValue>>,
    pub condition: Condition,
    pub message_template: SmolStr,
    pub where_policy: WherePolicy,
}

impl StatRule {
    pub fn builder(id: impl Into<SmolStr>, scope: Scope) -> StatRuleBuilder {
        StatRuleBuilder::new(id, scope)
    }

    /// Interpolates `${scope.property}` and `${options.key}` placeholders in `message_template`.
    pub fn format_message(&self, properties: &PropertyBag) -> String {
        let template = self.message_template.as_str();
        let mut result = String::with_capacity(template.len());
        let mut cursor = 0;

        while let Some(start_index) = template[cursor..].find("${") {
            let actual_start = cursor + start_index;
            result.push_str(&template[cursor..actual_start]);

            if let Some(end_index) = template[actual_start..].find('}') {
                let actual_end = actual_start + end_index;
                let placeholder = &template[actual_start + 2..actual_end];

                if let Some(value) = self.resolve_placeholder(placeholder, properties) {
                    result.push_str(&value);
                } else {
                    result.push_str(&template[actual_start..=actual_end]);
                }
                cursor = actual_end + 1;
            } else {
                result.push_str(&template[actual_start..]);
                cursor = template.len();
                break;
            }
        }

        if cursor < template.len() {
            result.push_str(&template[cursor..]);
        }

        result
    }

    fn resolve_placeholder(&self, placeholder: &str, properties: &PropertyBag) -> Option<String> {
        if let Some(option_key) = placeholder.strip_prefix("options.") {
            return self
                .options
                .as_ref()
                .and_then(|options_map| options_map.get(option_key))
                .map(|val| match val {
                    OptionValue::Int(n) => n.to_string(),
                    OptionValue::Float(f) => f.to_string(),
                    OptionValue::Str(s) => s.to_string(),
                    OptionValue::Bool(b) => b.to_string(),
                    OptionValue::StrList(list) => list.join(", "),
                });
        }

        if let Some((scope_prefix, field_name)) = placeholder.split_once('.')
            && let Some(scope) = Scope::parse(scope_prefix)
            && scope == self.scope
            && let Some(property_key) =
                crate::types::PropertyKey::from_scope_and_field(scope, field_name)
            && let Some(value) = properties.get(property_key)
        {
            return Some(match value {
                OptionValue::Int(n) => n.to_string(),
                OptionValue::Float(f) => f.to_string(),
                OptionValue::Str(s) => s.to_string(),
                OptionValue::Bool(b) => b.to_string(),
                OptionValue::StrList(list) => list.join(", "),
            });
        }

        None
    }

    /// Returns all property keys needed by this rule (from both the condition tree and message template).
    pub fn referenced_properties(&self) -> Vec<PropertyKey> {
        let mut props = self.condition.referenced_properties();
        let template = self.message_template.as_str();
        let mut cursor = 0;

        while let Some(start_index) = template[cursor..].find("${") {
            let actual_start = cursor + start_index;
            if let Some(end_index) = template[actual_start..].find('}') {
                let actual_end = actual_start + end_index;
                let placeholder = &template[actual_start + 2..actual_end];
                if let Some((scope_prefix, field_name)) = placeholder.split_once('.')
                    && let Some(scope) = Scope::parse(scope_prefix)
                    && let Some(key) = PropertyKey::from_scope_and_field(scope, field_name)
                    && !props.contains(&key)
                {
                    props.push(key);
                }
                cursor = actual_end + 1;
            } else {
                break;
            }
        }

        props
    }
}

/// Fluent builder for constructing `StatRule` instances programmatically.
pub struct StatRuleBuilder {
    id: SmolStr,
    report_id: Option<SmolStr>,
    severity: Severity,
    scope: Scope,
    options: Option<IndexMap<SmolStr, OptionValue>>,
    condition: Option<Condition>,
    message_template: SmolStr,
    where_policy: WherePolicy,
}

impl StatRuleBuilder {
    pub fn new(id: impl Into<SmolStr>, scope: Scope) -> Self {
        let id_smol: SmolStr = id.into();
        Self {
            id: id_smol.clone(),
            report_id: None,
            severity: Severity::Warning,
            scope,
            options: None,
            condition: None,
            message_template: SmolStr::default(),
            where_policy: WherePolicy::AllowOverride,
        }
    }

    pub fn report_id(mut self, report_id: impl Into<SmolStr>) -> Self {
        self.report_id = Some(report_id.into());
        self
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn where_policy(mut self, policy: WherePolicy) -> Self {
        self.where_policy = policy;
        self
    }

    pub fn option(mut self, key: impl Into<SmolStr>, value: OptionValue) -> Self {
        let map = self.options.get_or_insert_with(IndexMap::new);
        map.insert(key.into(), value);
        self
    }

    pub fn condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn message(mut self, template: impl Into<SmolStr>) -> Self {
        self.message_template = template.into();
        self
    }

    pub fn build(self) -> StatRule {
        let report_id = self.report_id.unwrap_or_else(|| self.id.clone());
        StatRule {
            id: self.id,
            report_id,
            severity: self.severity,
            scope: self.scope,
            options: self.options,
            condition: self.condition.expect("Condition is required for StatRule"),
            message_template: self.message_template,
            where_policy: self.where_policy,
        }
    }
}
