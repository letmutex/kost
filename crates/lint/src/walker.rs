//! Single-pass DFS tree-sitter AST traversal engine.

use crate::context::LintCtx;
use crate::extractor::{
    populate_class_properties, populate_file_properties, populate_function_properties,
    populate_import_properties, populate_lambda_properties, populate_property_properties,
};
use crate::registry::RuleRegistry;
use crate::types::{Diagnostic, PropertyKey, PropertyValue, Scope, TextRange};

/// Executes a single-pass AST traversal over source code and returns all detected diagnostics.
pub fn lint_source(source: &[u8], registry: &RuleRegistry) -> Vec<Diagnostic> {
    lint_source_with_path(source, None, registry)
}

/// Executes a single-pass AST traversal over source code with file path context.
pub fn lint_source_with_path(
    source: &[u8],
    file_path: Option<&str>,
    registry: &RuleRegistry,
) -> Vec<Diagnostic> {
    lint_source_with_path_and_config(source, file_path, registry, None)
}

/// Executes a single-pass AST traversal with optional file path and EditorConfig context.
pub fn lint_source_with_path_and_config(
    source: &[u8],
    file_path: Option<&str>,
    registry: &RuleRegistry,
    editorconfig: Option<&common::editorconfig::ResolvedEditorConfig>,
) -> Vec<Diagnostic> {
    let mut parser = tree_sitter::Parser::new();
    let language: tree_sitter::Language =
        unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let root_node = tree.root_node();
    let suppression_locator = crate::suppression::SuppressionLocator::build(root_node, source);
    let mut ctx = LintCtx::new(source, registry, &suppression_locator)
        .with_file_path(file_path)
        .with_editorconfig(editorconfig);

    // Enter root File scope
    let file_range = TextRange::from_node(root_node);
    ctx.scope_tracker.push(Scope::File, file_range);
    populate_file_properties(&root_node, &mut ctx);

    // Walk DFS
    let mut cursor = root_node.walk();
    walk_cursor(&mut cursor, source, &mut ctx);

    // Finalize File scope
    if let Some(mut file_frame) = ctx.scope_tracker.pop() {
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
        evaluate_scope_stat_rules(
            &file_frame.scope,
            &file_frame.range,
            &file_frame.properties,
            &mut ctx,
        );
    }

    ctx.diagnostics
}

fn walk_cursor<'a>(cursor: &mut tree_sitter::TreeCursor<'a>, source: &[u8], ctx: &mut LintCtx<'a>) {
    let node = cursor.node();
    ctx.ancestors.push(node);

    if ctx
        .registry
        .needed_properties
        .contains(&PropertyKey::FileMaxIndentSpaces)
    {
        update_max_indent_if_needed(node, source, ctx);
    }

    let kind = node.kind();
    let entered_string = if ctx.registry.needs_file_exclusions {
        collect_exclusions_if_matching(node, kind, source, ctx)
    } else {
        false
    };
    let opened_scope = enter_scope_if_matching(node, source, ctx);

    // 1. Dispatch pre-order AST rules
    dispatch_pre_rules(node, kind, source, ctx);

    // 2. Recurse down children using cursor
    if cursor.goto_first_child() {
        let saved_prev = ctx.prev_sibling;
        let mut prev_sibling = None;
        loop {
            ctx.prev_sibling = prev_sibling;
            walk_cursor(cursor, source, ctx);
            prev_sibling = Some(cursor.node());
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
        ctx.prev_sibling = saved_prev;
    }

    // 3. Dispatch post-order AST rules
    dispatch_post_rules(node, kind, source, ctx);

    // 4. Exit scope and evaluate statistical rules for this scope
    if let Some(scope) = opened_scope
        && let Some(frame) = ctx.scope_tracker.pop()
    {
        evaluate_scope_stat_rules(&scope, &frame.range, &frame.properties, ctx);
    }

    if entered_string {
        ctx.in_string_literal = false;
    }

    ctx.ancestors.pop();
}

#[inline]
fn dispatch_pre_rules(node: tree_sitter::Node, kind: &str, source: &[u8], ctx: &mut LintCtx) {
    if let Some(rule_indices) = ctx.registry.ast_dispatch.get(kind) {
        for &index in rule_indices {
            if ctx.is_rule_disabled(index) {
                continue;
            }
            let rule = &ctx.registry.ast_rules[index];
            ctx.active_rule_index = index;
            ctx.active_severity = ctx.registry.ast_severities[index];
            ctx.active_ignores_suppressions = rule.ignores_suppressions();
            rule.check_node(node, source, ctx);
        }
    }

    for &index in &ctx.registry.ast_catch_all {
        if ctx.is_rule_disabled(index) {
            continue;
        }
        let rule = &ctx.registry.ast_rules[index];
        ctx.active_rule_index = index;
        ctx.active_severity = ctx.registry.ast_severities[index];
        ctx.active_ignores_suppressions = rule.ignores_suppressions();
        rule.check_node(node, source, ctx);
    }
}

#[inline]
fn dispatch_post_rules(node: tree_sitter::Node, kind: &str, source: &[u8], ctx: &mut LintCtx) {
    if let Some(rule_indices) = ctx.registry.ast_dispatch.get(kind) {
        for &index in rule_indices {
            if ctx.is_rule_disabled(index) {
                continue;
            }
            let rule = &ctx.registry.ast_rules[index];
            ctx.active_rule_index = index;
            ctx.active_severity = ctx.registry.ast_severities[index];
            ctx.active_ignores_suppressions = rule.ignores_suppressions();
            rule.after_node(node, source, ctx);
        }
    }

    for &index in &ctx.registry.ast_catch_all {
        if ctx.is_rule_disabled(index) {
            continue;
        }
        let rule = &ctx.registry.ast_rules[index];
        ctx.active_rule_index = index;
        ctx.active_severity = ctx.registry.ast_severities[index];
        ctx.active_ignores_suppressions = rule.ignores_suppressions();
        rule.after_node(node, source, ctx);
    }
}

fn enter_scope_if_matching(
    node: tree_sitter::Node,
    source: &[u8],
    ctx: &mut LintCtx,
) -> Option<Scope> {
    let kind = node.kind();
    let range = TextRange::from_node(node);

    match kind {
        "function_declaration" => {
            ctx.scope_tracker.push(Scope::Function, range);
            populate_function_properties(node, source, ctx);
            Some(Scope::Function)
        }
        "class_declaration" | "object_declaration" => {
            ctx.scope_tracker.push(Scope::Class, range);
            populate_class_properties(node, source, ctx);
            Some(Scope::Class)
        }
        "property_declaration" => {
            ctx.scope_tracker.push(Scope::Property, range);
            populate_property_properties(node, source, ctx);
            Some(Scope::Property)
        }
        "lambda_literal" => {
            ctx.scope_tracker.push(Scope::Lambda, range);
            populate_lambda_properties(node, ctx);
            Some(Scope::Lambda)
        }
        "import_header" => {
            ctx.scope_tracker.push(Scope::Import, range);
            populate_import_properties(node, source, ctx);
            Some(Scope::Import)
        }
        _ => None,
    }
}

fn evaluate_scope_stat_rules(
    scope: &Scope,
    range: &TextRange,
    properties: &crate::condition::PropertyBag,
    ctx: &mut LintCtx,
) {
    if let Some(rules) = ctx.registry.stat_dispatch.get(scope) {
        for rule in rules {
            if rule.condition.matches(properties, rule.options.as_ref()) {
                if ctx
                    .suppression_locator
                    .is_suppressed(&rule.report_id, range)
                {
                    continue;
                }
                let message = rule.format_message(properties);
                ctx.diagnostics.push(Diagnostic {
                    rule_id: rule.report_id.clone(),
                    severity: rule.severity,
                    range: *range,
                    message,
                });
            }
        }
    }
}

fn update_max_indent_if_needed(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let kind = node.kind();
    if kind == "source_file" {
        return;
    }

    let is_comment = matches!(kind, "line_comment" | "multiline_comment" | "comment");
    let is_string = matches!(kind, "string_literal" | "character_literal");

    let row = node.start_position().row as isize;
    if !is_comment && row > ctx.last_indent_row {
        let indent = compute_line_indent(source, node.start_byte());
        if indent > ctx.max_indent_spaces {
            ctx.max_indent_spaces = indent;
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

fn compute_line_indent(source: &[u8], node_start_byte: usize) -> i64 {
    let mut line_start = node_start_byte;
    while line_start > 0 && source[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    let mut indent: i64 = 0;
    for &byte in &source[line_start..] {
        match byte {
            b' ' => indent += 1,
            b'\t' => indent += 4,
            b'\r' => {}
            _ => break,
        }
    }
    indent
}

#[inline]
fn collect_exclusions_if_matching(
    node: tree_sitter::Node,
    kind: &str,
    source: &[u8],
    ctx: &mut LintCtx,
) -> bool {
    let entered_string = if !ctx.in_string_literal
        && matches!(kind, "string_literal" | "multiline_string_literal")
    {
        ctx.in_string_literal = true;
        let range = (node.start_byte(), node.end_byte());
        ctx.all_string_and_comment_ranges.push(range);
        if node.start_position().row != node.end_position().row {
            ctx.multiline_string_ranges.push(range);
            ctx.multiline_string_and_comment_ranges.push(range);
            ctx.max_line_length_exclusions.push(range);
        }
        true
    } else {
        false
    };

    if !ctx.in_string_literal {
        if matches!(kind, "character_literal" | "line_comment") {
            ctx.all_string_and_comment_ranges
                .push((node.start_byte(), node.end_byte()));
        } else if matches!(kind, "multiline_comment" | "comment") {
            let range = (node.start_byte(), node.end_byte());
            ctx.all_string_and_comment_ranges.push(range);
            if node.start_position().row != node.end_position().row {
                ctx.multiline_string_and_comment_ranges.push(range);
            }
            if let Some(slice) = source.get(range.0..range.1)
                && crate::util::is_kdoc_bytes(slice)
            {
                ctx.max_line_length_exclusions.push(range);
            }
        }
    }

    entered_string
}
