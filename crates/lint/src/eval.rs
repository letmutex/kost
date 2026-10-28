//! Evaluation of statistical lint rules against lexical scopes.

use crate::context::{LintCtx, ScopeFrame, StackPropertyResolver};
use crate::extractor::populate_file_properties;
use crate::types::{Diagnostic, PropertyKey, PropertyValue, ReportTarget, Scope, TextRange};
use smallvec::SmallVec;

pub fn init_file_scope(root_node: tree_sitter::Node, ctx: &mut LintCtx) {
    let file_range = TextRange::from_node(root_node);
    ctx.scope_tracker.push(Scope::File, file_range);
    if let Some(frame) = ctx.scope_tracker.current_mut() {
        let start = TextRange::new(
            file_range.start_line,
            file_range.start_col,
            file_range.start_line,
            file_range.start_col,
        );
        let end = TextRange::new(
            file_range.end_line,
            file_range.end_col,
            file_range.end_line,
            file_range.end_col,
        );
        frame.insert_target_anchor(ReportTarget::FileStart, start);
        frame.insert_target_anchor(ReportTarget::FileEnd, end);
    }
    populate_file_properties(&root_node, ctx);
}

pub fn finalize_file_scope(ctx: &mut LintCtx) {
    if let Some(mut file_frame) = ctx.scope_tracker.pop() {
        if let Some(range) = ctx.max_indent_range {
            file_frame.insert_target_anchor(ReportTarget::FileMaxIndentSpacesAt, range);
        }
        if ctx
            .registry
            .needed_properties
            .contains(&PropertyKey::FileMaxIndentSpaces)
        {
            file_frame.properties.insert(
                PropertyKey::FileMaxIndentSpaces,
                PropertyValue::Int(ctx.max_indent_spaces),
            );
        }
        if ctx
            .registry
            .needed_properties
            .contains(&PropertyKey::FileFunctionsCount)
        {
            file_frame.properties.insert(
                PropertyKey::FileFunctionsCount,
                PropertyValue::Int(ctx.file_functions_count),
            );
        }
        evaluate_scope_stat_rules(&file_frame.scope, &file_frame, ctx);
    }
}

pub fn update_max_indent_if_needed(
    node: tree_sitter::Node,
    kind: &str,
    source: &[u8],
    ctx: &mut LintCtx,
) {
    if kind == "source_file" {
        return;
    }

    let is_comment = matches!(kind, "line_comment" | "multiline_comment" | "comment");
    let is_string = matches!(kind, "string_literal" | "character_literal");

    let row = node.start_position().row as isize;
    if !is_comment && row > ctx.last_indent_row {
        let (indent, ws_len) =
            crate::util::compute_line_indent_and_col_width(source, node.start_byte());
        if indent > ctx.max_indent_spaces {
            ctx.max_indent_spaces = indent;
            let line = (row + 1) as u32;
            ctx.max_indent_range = Some(TextRange::new(line, 1, line, ws_len + 1));
        }
        ctx.last_indent_row = row;
    }

    if is_string {
        let end_row = node.end_position().row as isize;
        if end_row > ctx.last_indent_row {
            ctx.last_indent_row = end_row;
        }
    }
}

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
