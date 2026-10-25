//! Configuration loading, include graph resolution, and rule merging.

pub mod error;
pub mod merge;
pub mod operator;
pub mod parser;
pub mod schema;
pub mod template;
pub mod validate;

use crate::config::error::ConfigError;
use crate::config::merge::{ExportedRuleset, RuleLookup, RuleMerger};
use crate::config::parser::RuleParser;
use crate::config::schema::{ConfigFile, RawInclude};
use crate::registry::RuleRegistry;
use crate::rule::StatRule;
use crate::types::{Severity, WherePolicy};
use ahash::AHashMap;
use smol_str::SmolStr;
use std::path::{Path, PathBuf};

const STAT_YAML: &str = include_str!("../rules/stat.yaml");

const VIRTUAL_STANDARD: &str = "standard";
const VIRTUAL_STATISTICS: &str = "statistics";

/// Parses an inline include string, checking if it is a qualified rule import like `<target>:<rule>`.
pub fn parse_qualified_include(s: &str) -> Option<(&str, &str)> {
    let (target, rule) = s.rsplit_once(':')?;
    if target.is_empty() || rule.is_empty() {
        return None;
    }
    // Exclude Windows drive root paths like "C:\..." or "C:/..." where ':' is the drive separator.
    if target.len() == 1
        && target.as_bytes()[0].is_ascii_alphabetic()
        && (rule.starts_with('\\') || rule.starts_with('/'))
    {
        return None;
    }
    Some((target, rule))
}

/// Configuration loader that resolves include DAGs and merges rule overrides.
pub struct ConfigLoader {
    memoized_files: AHashMap<PathBuf, ExportedRuleset>,
    known_basenames: AHashMap<String, PathBuf>,
    memoized_virtual: AHashMap<&'static str, ExportedRuleset>,
    visited_chain: Vec<String>,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            memoized_files: AHashMap::new(),
            known_basenames: AHashMap::new(),
            memoized_virtual: AHashMap::new(),
            visited_chain: Vec::new(),
        }
    }

    pub fn load_file(path: &Path) -> Result<RuleRegistry, ConfigError> {
        let mut loader = Self::new();
        let exported = loader.get_or_load_file(path)?;
        Ok(exported.into_registry())
    }

    pub fn load_string(content: &str, file_name: &str) -> Result<RuleRegistry, ConfigError> {
        let mut loader = Self::new();
        loader
            .known_basenames
            .insert(file_name.to_string(), PathBuf::from(file_name));
        let exported = loader.parse_and_evaluate(content, file_name, None)?;
        Ok(exported.into_registry())
    }

    fn get_or_load_virtual(&mut self, scope: &'static str) -> Result<ExportedRuleset, ConfigError> {
        if let Some(exported) = self.memoized_virtual.get(scope) {
            return Ok(exported.clone());
        }

        let exported = match scope {
            VIRTUAL_STANDARD => {
                let mut merger = RuleMerger::new();
                for id in crate::rules::standard::standard_ast_rule_ids() {
                    let full_id = crate::concat_smol_str!("standard:", *id);
                    merger.insert_ast(full_id, Severity::Warning);
                }
                merger.into_exported()
            }
            VIRTUAL_STATISTICS => self.build_virtual_ruleset(VIRTUAL_STATISTICS, STAT_YAML)?,
            _ => unreachable!("Invalid virtual ruleset: {scope}"),
        };

        self.memoized_virtual.insert(scope, exported.clone());
        Ok(exported)
    }

    fn build_virtual_ruleset(
        &self,
        scope: &str,
        stat_yaml: &str,
    ) -> Result<ExportedRuleset, ConfigError> {
        let mut merger = RuleMerger::new();

        let parsed: ConfigFile =
            serde_yaml::from_str(stat_yaml).map_err(|e| ConfigError::Yaml {
                file: scope.to_string(),
                source: e,
            })?;
        let lint = parsed.lint.ok_or_else(|| ConfigError::MissingLintSection {
            file: scope.to_string(),
        })?;

        for raw in lint.rules {
            let full_id = crate::concat_smol_str!(scope, ":", &raw.id);
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

            let policy = WherePolicy::from_allow_bool(raw.allow_where_override.unwrap_or(true));
            let severity = raw
                .severity
                .as_deref()
                .and_then(Severity::parse)
                .unwrap_or(Severity::Warning);

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

            merger.insert_stat(stat_rule);
        }

        Ok(merger.into_exported())
    }

    fn get_or_load_file(&mut self, path: &Path) -> Result<ExportedRuleset, ConfigError> {
        let canonical = path.canonicalize().map_err(|e| ConfigError::Io {
            file: path.display().to_string(),
            source: e,
        })?;

        if let Some(exported) = self.memoized_files.get(&canonical) {
            return Ok(exported.clone());
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config.yaml")
            .to_string();

        if let Some(existing_path) = self.known_basenames.get(&file_name) {
            if existing_path != &canonical {
                return Err(ConfigError::AmbiguousScope {
                    filename: file_name,
                    first_path: existing_path.display().to_string(),
                    second_path: canonical.display().to_string(),
                });
            }
        } else {
            self.known_basenames
                .insert(file_name.clone(), canonical.clone());
        }

        if self.visited_chain.contains(&file_name) {
            let mut chain = self.visited_chain.clone();
            chain.push(file_name);
            return Err(ConfigError::CircularInclude {
                chain: chain.join(" -> "),
            });
        }

        self.visited_chain.push(file_name.clone());

        let content = std::fs::read_to_string(&canonical).map_err(|e| ConfigError::Io {
            file: canonical.display().to_string(),
            source: e,
        })?;

        let parent_dir = canonical.parent().map(|p| p.to_path_buf());
        let exported = self.parse_and_evaluate(&content, &file_name, parent_dir.as_deref())?;

        self.visited_chain.pop();
        self.memoized_files.insert(canonical, exported.clone());
        Ok(exported)
    }

    fn parse_and_evaluate(
        &mut self,
        content: &str,
        file_name: &str,
        base_dir: Option<&Path>,
    ) -> Result<ExportedRuleset, ConfigError> {
        let trimmed = content.trim();
        if trimmed.is_empty() || trimmed.lines().all(|line| line.trim().starts_with('#')) {
            return Err(ConfigError::MissingLintSection {
                file: file_name.to_string(),
            });
        }

        let config: ConfigFile = serde_yaml::from_str(content).map_err(|e| ConfigError::Yaml {
            file: file_name.to_string(),
            source: e,
        })?;

        let lint = config.lint.ok_or_else(|| ConfigError::MissingLintSection {
            file: file_name.to_string(),
        })?;

        let mut local_merger = RuleMerger::new();

        for raw_include in lint.includes {
            self.process_include(raw_include, &mut local_merger, base_dir)?;
        }

        for raw_rule in lint.rules {
            local_merger.process_entry(raw_rule, file_name)?;
        }

        Ok(local_merger.into_exported())
    }

    fn process_include(
        &mut self,
        raw_include: RawInclude,
        target_merger: &mut RuleMerger,
        base_dir: Option<&Path>,
    ) -> Result<(), ConfigError> {
        let (target, rules) = match raw_include {
            RawInclude::Simple(s) => match parse_qualified_include(&s) {
                Some((target_str, rule_str)) => {
                    (target_str.to_string(), Some(vec![rule_str.to_string()]))
                }
                None => (s, None),
            },
            RawInclude::Structured { path, rules } => {
                if rules.is_empty() {
                    return Err(ConfigError::EmptyIncludeRules { target: path });
                }
                (path, Some(rules))
            }
        };

        let exported = if target == VIRTUAL_STANDARD || target == VIRTUAL_STATISTICS {
            let static_str = if target == VIRTUAL_STANDARD {
                VIRTUAL_STANDARD
            } else {
                VIRTUAL_STATISTICS
            };
            self.get_or_load_virtual(static_str)?
        } else {
            let full_path = match base_dir {
                Some(dir) => dir.join(&target),
                None => PathBuf::from(&target),
            };
            self.get_or_load_file(&full_path)?
        };

        match rules {
            None => {
                for (id, rule) in &exported.rules {
                    target_merger.insert_merged(id.clone(), rule.clone());
                }
            }
            Some(rule_list) => {
                for rule_name in rule_list {
                    let (id, rule) = match exported.find_rule(&rule_name) {
                        RuleLookup::Found(id, rule) => (id, rule),
                        RuleLookup::Ambiguous(candidates) => {
                            return Err(ConfigError::AmbiguousIncludedRule {
                                target: target.clone(),
                                rule: rule_name.clone(),
                                candidates: candidates.join(", "),
                            });
                        }
                        RuleLookup::NotFound => {
                            return Err(ConfigError::UnknownIncludedRule {
                                target: target.clone(),
                                rule: rule_name.clone(),
                            });
                        }
                    };
                    target_merger.insert_merged(id.clone(), rule.clone());
                }
            }
        }

        Ok(())
    }
}
