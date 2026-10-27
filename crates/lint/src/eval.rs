//! Evaluation of statistical lint rules against lexical scopes.

use crate::context::{LintCtx, ScopeFrame, StackPropertyResolver};
use crate::types::{Diagnostic, Scope, TextRange};
use smallvec::SmallVec;

/// Evaluates all statistical rules registered for `scope` against the properties of `frame`.
pub fn evaluate_scope_stat_rules(scope: &Scope, frame: &ScopeFrame, ctx: &mut LintCtx) {
    let Some(rules) = ctx.registry.stat_dispatch.get(scope) else {
        return;
    };

    let resolver = StackPropertyResolver {
        current: frame,
        tracker: &ctx.scope_tracker,
    };

    for rule in rules {
        if !rule.condition.matches(&resolver, rule.options.as_ref()) {
            continue;
        }

        let report_ranges: SmallVec<[TextRange; 1]> = if rule.report_targets.is_empty() {
            smallvec::smallvec![frame.range]
        } else {
            let mut targets = SmallVec::with_capacity(rule.report_targets.len());
            for target in &rule.report_targets {
                if let Some(range) = resolver.resolve_target_anchor(*target)
                    && !targets.contains(&range)
                {
                    targets.push(range);
                }
            }
            if targets.is_empty() {
                smallvec::smallvec![frame.range]
            } else {
                targets
            }
        };

        let is_suppressed = ctx
            .suppression_locator
            .is_suppressed(&rule.report_id, &frame.range)
            || report_ranges.iter().any(|range| {
                ctx.suppression_locator
                    .is_suppressed(&rule.report_id, range)
            });

        if is_suppressed {
            continue;
        }

        let message = rule.format_message(&resolver);
        ctx.diagnostics.push(Diagnostic {
            rule_id: rule.report_id.clone(),
            severity: rule.severity,
            check_range: frame.range,
            report_ranges,
            message,
        });
    }
}
