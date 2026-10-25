//! Single-pass DFS tree-sitter AST traversal engine.

use crate::context::LintCtx;
use crate::extractor::{enter_scope_if_matching, populate_file_properties};
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
        evaluate_scope_stat_rules(
            &file_frame.scope,
            &file_frame.range,
            &file_frame.properties,
            &mut ctx,
        );
    }

    ctx.diagnostics
}

struct WalkerFrame<'a> {
    node: tree_sitter::Node<'a>,
    opened_scope: Option<Scope>,
    entered_string: bool,
    saved_prev_sibling: Option<tree_sitter::Node<'a>>,
}

fn walk_cursor<'a>(cursor: &mut tree_sitter::TreeCursor<'a>, source: &[u8], ctx: &mut LintCtx<'a>) {
    let mut stack: Vec<WalkerFrame<'a>> = Vec::with_capacity(64);

    loop {
        let curr_node = cursor.node();
        ctx.ancestors.push(curr_node);

        if ctx
            .registry
            .needed_properties
            .contains(&PropertyKey::FileMaxIndentSpaces)
        {
            update_max_indent_if_needed(curr_node, source, ctx);
        }

        let kind = curr_node.kind();
        let entered_string = if ctx.registry.needs_file_exclusions {
            collect_exclusions_if_matching(curr_node, kind, source, ctx)
        } else {
            false
        };
        let opened_scope = enter_scope_if_matching(curr_node, source, ctx);
        if opened_scope == Some(Scope::Function) {
            ctx.file_functions_count += 1;
        }

        // 1. Dispatch pre-order AST rules
        dispatch_pre_rules(curr_node, kind, source, ctx);

        // 2. Try to descend to first child
        if cursor.goto_first_child() {
            stack.push(WalkerFrame {
                node: curr_node,
                opened_scope,
                entered_string,
                saved_prev_sibling: ctx.prev_sibling,
            });
            ctx.prev_sibling = None;
            continue;
        }

        // 3. Leaf node: dispatch post-order exit logic
        exit_node(curr_node, kind, opened_scope, entered_string, source, ctx);

        // 4. Advance to next sibling or ascend to parent
        let mut last_node = curr_node;
        loop {
            if stack.is_empty() {
                return;
            }

            if cursor.goto_next_sibling() {
                ctx.prev_sibling = Some(last_node);
                break;
            }
            cursor.goto_parent();
            let parent = stack.pop().unwrap();
            ctx.prev_sibling = parent.saved_prev_sibling;
            exit_node(
                parent.node,
                parent.node.kind(),
                parent.opened_scope,
                parent.entered_string,
                source,
                ctx,
            );
            last_node = parent.node;
        }
    }
}

#[inline]
fn exit_node<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
    opened_scope: Option<Scope>,
    entered_string: bool,
    source: &[u8],
    ctx: &mut LintCtx<'a>,
) {
    // Dispatch post-order AST rules
    dispatch_post_rules(node, kind, source, ctx);

    // Exit scope and evaluate statistical rules for this scope
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
fn dispatch_rules<F>(kind: &str, ctx: &mut LintCtx, mut apply: F)
where
    F: FnMut(&dyn crate::rule::AstRule, &mut LintCtx),
{
    let specific = ctx.registry.ast_dispatch.get(kind).into_iter().flatten();
    for &index in specific.chain(&ctx.registry.ast_catch_all) {
        if ctx.is_rule_disabled(index) {
            continue;
        }
        let rule = &ctx.registry.ast_rules[index];
        ctx.active_rule_index = index;
        ctx.active_severity = ctx.registry.ast_severities[index];
        ctx.active_ignores_suppressions = rule.ignores_suppressions();
        apply(rule.as_ref(), ctx);
    }
}

#[inline]
fn dispatch_pre_rules(node: tree_sitter::Node, kind: &str, source: &[u8], ctx: &mut LintCtx) {
    dispatch_rules(kind, ctx, |rule, c| rule.check_node(node, source, c));
}

#[inline]
fn dispatch_post_rules(node: tree_sitter::Node, kind: &str, source: &[u8], ctx: &mut LintCtx) {
    dispatch_rules(kind, ctx, |rule, c| rule.after_node(node, source, c));
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
        let indent = crate::util::compute_line_indent(source, node.start_byte());
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
        } else if kind == "simple_identifier" {
            let start = node.start_byte();
            let end = node.end_byte();
            if end >= start + 2
                && source.get(start) == Some(&b'`')
                && source.get(end - 1) == Some(&b'`')
            {
                ctx.all_string_and_comment_ranges.push((start, end));
            }
        }
    }

    entered_string
}
