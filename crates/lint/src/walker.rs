//! Single-pass DFS tree-sitter AST traversal engine.

use crate::context::LintCtx;
use crate::eval::{
    evaluate_scope_stat_rules, finalize_file_scope, init_file_scope, update_max_indent_if_needed,
};
use crate::extractor::enter_scope_if_matching;
use crate::registry::RuleRegistry;
use crate::types::{Diagnostic, PropertyKey, Scope};
use crate::util::{is_declaration_kind, node_kind};

thread_local! {
    static THREAD_PARSER: std::cell::RefCell<tree_sitter::Parser> = std::cell::RefCell::new({
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language =
            unsafe { std::mem::transmute((tree_sitter_kotlin::LANGUAGE.into_raw())()) };
        parser
            .set_language(&language)
            .expect("Failed to load Kotlin tree-sitter grammar");
        parser
    });
}

fn parse_kotlin_source(source: &[u8]) -> Option<tree_sitter::Tree> {
    THREAD_PARSER.with(|parser| parser.borrow_mut().parse(source, None))
}

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
    let Some(tree) = parse_kotlin_source(source) else {
        return Vec::new();
    };

    let root_node = tree.root_node();
    let suppression_locator = crate::suppression::SuppressionLocator::build(root_node, source);
    let mut ctx = LintCtx::new(source, registry, &suppression_locator)
        .with_file_path(file_path)
        .with_editorconfig(editorconfig);

    let has_stat_rules = !registry.stat_dispatch.is_empty();
    if has_stat_rules {
        init_file_scope(root_node, &mut ctx);
    }

    // Walk DFS
    let mut cursor = root_node.walk();
    walk_cursor(&mut cursor, source, &mut ctx);

    if has_stat_rules {
        finalize_file_scope(&mut ctx);
    }
    ctx.diagnostics
}

struct WalkerFrame<'a> {
    node: tree_sitter::Node<'a>,
    kind: &'static str,
    opened_scope: Option<Scope>,
    entered_string: bool,
    saved_prev_sibling: Option<tree_sitter::Node<'a>>,
    saved_prev_named_sibling: Option<tree_sitter::Node<'a>>,
    saved_prev_meaningful_sibling: Option<tree_sitter::Node<'a>>,
    saved_prev_declaration_sibling: Option<tree_sitter::Node<'a>>,
}

#[inline]
fn record_completed_sibling<'a>(
    last_node: tree_sitter::Node<'a>,
    last_kind: &str,
    ctx: &mut LintCtx<'a>,
) {
    ctx.prev_sibling = Some(last_node);
    let is_named = last_node.is_named();
    if is_named {
        ctx.prev_named_sibling = Some(last_node);
        if is_declaration_kind(last_kind) {
            ctx.prev_declaration_sibling = Some(last_node);
        }
    }
    if is_named || matches!(last_kind, "line_comment" | "multiline_comment") {
        ctx.prev_meaningful_sibling = Some(last_node);
    }
}

fn walk_cursor<'a>(cursor: &mut tree_sitter::TreeCursor<'a>, source: &[u8], ctx: &mut LintCtx<'a>) {
    let mut stack: Vec<WalkerFrame<'a>> = Vec::with_capacity(64);
    let has_stat_rules = !ctx.registry.stat_dispatch.is_empty();
    let needs_max_indent = has_stat_rules
        && ctx
            .registry
            .needed_properties
            .contains(&PropertyKey::FileMaxIndentSpaces);

    loop {
        let curr_node = cursor.node();
        let kind = node_kind(curr_node);
        ctx.ancestors.push(curr_node);

        if needs_max_indent {
            update_max_indent_if_needed(curr_node, kind, source, ctx);
        }

        let entered_string = if ctx.registry.needs_file_exclusions {
            collect_exclusions_if_matching(curr_node, kind, source, ctx)
        } else {
            false
        };
        let opened_scope = if has_stat_rules {
            let scope = enter_scope_if_matching(curr_node, kind, source, ctx);
            if scope == Some(Scope::Function) {
                ctx.file_functions_count += 1;
            }
            scope
        } else {
            None
        };

        // 1. Dispatch pre-order AST rules
        dispatch_pre_rules(curr_node, kind, source, ctx);

        // 2. Try to descend to first child
        if cursor.goto_first_child() {
            stack.push(WalkerFrame {
                node: curr_node,
                kind,
                opened_scope,
                entered_string,
                saved_prev_sibling: ctx.prev_sibling.take(),
                saved_prev_named_sibling: ctx.prev_named_sibling.take(),
                saved_prev_meaningful_sibling: ctx.prev_meaningful_sibling.take(),
                saved_prev_declaration_sibling: ctx.prev_declaration_sibling.take(),
            });
            continue;
        }

        // 3. Leaf node: dispatch post-order exit logic
        exit_node(curr_node, kind, opened_scope, entered_string, source, ctx);

        // 4. Advance to next sibling or ascend to parent
        if !advance_or_ascend(cursor, &mut stack, curr_node, kind, source, ctx) {
            return;
        }
    }
}

#[inline]
fn advance_or_ascend<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    stack: &mut Vec<WalkerFrame<'a>>,
    mut last_node: tree_sitter::Node<'a>,
    mut last_kind: &'static str,
    source: &[u8],
    ctx: &mut LintCtx<'a>,
) -> bool {
    loop {
        if stack.is_empty() {
            return false;
        }

        if cursor.goto_next_sibling() {
            record_completed_sibling(last_node, last_kind, ctx);
            return true;
        }
        cursor.goto_parent();
        let parent = stack.pop().unwrap();
        ctx.prev_sibling = parent.saved_prev_sibling;
        ctx.prev_named_sibling = parent.saved_prev_named_sibling;
        ctx.prev_meaningful_sibling = parent.saved_prev_meaningful_sibling;
        ctx.prev_declaration_sibling = parent.saved_prev_declaration_sibling;
        exit_node(
            parent.node,
            parent.kind,
            parent.opened_scope,
            parent.entered_string,
            source,
            ctx,
        );
        last_node = parent.node;
        last_kind = parent.kind;
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
        evaluate_scope_stat_rules(&scope, &frame, ctx);
    }

    if entered_string {
        ctx.in_string_literal = false;
    }

    ctx.ancestors.pop();
}

#[inline]
fn run_rule_indices<F>(indices: &[usize], ctx: &mut LintCtx, apply: &mut F)
where
    F: FnMut(&dyn crate::rule::AstRule, &mut LintCtx),
{
    for &index in indices {
        if ctx.is_rule_disabled(index) {
            continue;
        }
        let rule = &ctx.registry.ast_rules[index];
        ctx.active_rule_index = index;
        ctx.active_severity = ctx.registry.ast_severities[index];
        ctx.active_ignores_suppressions = ctx.registry.ast_ignores_suppressions[index];
        apply(rule.as_ref(), ctx);
    }
}

#[inline]
fn dispatch_pre_rules(node: tree_sitter::Node, kind: &str, source: &[u8], ctx: &mut LintCtx) {
    let mut apply =
        |rule: &dyn crate::rule::AstRule, c: &mut LintCtx| rule.check_node(node, source, c);
    let kind_id = node.kind_id() as usize;
    if let Some(specific) = ctx.registry.ast_dispatch_by_id.get(kind_id) {
        if !specific.is_empty() {
            run_rule_indices(specific, ctx, &mut apply);
        }
    } else if let Some(specific) = ctx.registry.ast_dispatch.get(kind) {
        run_rule_indices(specific, ctx, &mut apply);
    }
    if !ctx.registry.ast_catch_all.is_empty() {
        let catch_all = ctx.registry.ast_catch_all.clone();
        run_rule_indices(&catch_all, ctx, &mut apply);
    }
}

#[inline]
fn dispatch_post_rules(node: tree_sitter::Node, kind: &str, source: &[u8], ctx: &mut LintCtx) {
    let kind_id = node.kind_id() as usize;
    let has_specific = ctx
        .registry
        .ast_post_dispatch_by_id
        .get(kind_id)
        .is_some_and(|list| !list.is_empty());
    if !has_specific
        && ctx.registry.ast_post_catch_all.is_empty()
        && kind_id < ctx.registry.ast_post_dispatch_by_id.len()
    {
        return;
    }
    let mut apply =
        |rule: &dyn crate::rule::AstRule, c: &mut LintCtx| rule.after_node(node, source, c);
    if let Some(specific) = ctx.registry.ast_post_dispatch_by_id.get(kind_id) {
        if !specific.is_empty() {
            run_rule_indices(specific, ctx, &mut apply);
        }
    } else if let Some(specific) = ctx.registry.ast_dispatch.get(kind) {
        run_rule_indices(specific, ctx, &mut apply);
    }
    if !ctx.registry.ast_post_catch_all.is_empty() {
        let catch_all = ctx.registry.ast_post_catch_all.clone();
        run_rule_indices(&catch_all, ctx, &mut apply);
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
