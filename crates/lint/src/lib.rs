//! Kotlin linting engine for kost.

pub mod cli;
pub mod condition;
pub mod config;
pub mod context;
pub mod extractor;
pub mod linter;
pub mod property;
pub mod registry;
pub mod rule;
pub mod rules;
pub mod smart_lint;
pub mod snippet;
pub mod suppression;
pub mod types;
pub mod util;
pub mod walker;

pub use cli::run;
pub use condition::{CondOp, Condition, PropertyBag, PropertyCondition, PropertyLookup};
pub use config::ConfigLoader;
pub use config::error::ConfigError;
pub use context::{LintCtx, ScopeFrame, ScopeTracker, StackPropertyResolver};
pub use linter::{LintReport, Linter};
pub use registry::{RuleRegistry, RuleRegistryBuilder};
pub use rule::{AstRule, StatRule};
pub use suppression::{SuppressionHint, SuppressionLocator};
pub use types::{
    Diagnostic, Numeric, Operand, OptionValue, PropertyKey, PropertyValue, Scope, Severity,
    TextRange, WherePolicy,
};
pub use walker::{lint_source, lint_source_with_path};
