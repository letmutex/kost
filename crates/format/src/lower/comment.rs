use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::binary::is_binary_expression_kind;
use crate::lower::context::LowerContext;
use crate::rules::comments::{format_kdoc, normalize_line_comment};

pub(crate) fn compute_comment_indent<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> usize {
    let mut indent = 0;
    let mut curr = node.parent();
    while let Some(parent_node) = curr {
        let kind = parent_node.kind();
        if matches!(
            kind,
            "class_body"
                | "function_body"
                | "control_structure_body"
                | "try_expression"
                | "catch_block"
                | "finally_block"
                | "anonymous_initializer"
                | "getter"
                | "setter"
                | "lambda_literal"
                | "when_expression"
                | "secondary_constructor"
                | "enum_class_body"
        ) {
            indent += ctx.options.block_indent;
        } else if matches!(kind, "value_arguments") {
            indent += ctx.options.continuation_indent;
        } else if is_binary_expression_kind(kind) {
            indent += ctx.options.continuation_indent;
            let mut top = parent_node;
            while let Some(parent) = top.parent() {
                if is_binary_expression_kind(parent.kind()) {
                    top = parent;
                } else {
                    break;
                }
            }
            curr = Some(top);
        } else if kind == "property_declaration"
            && parent_node
                .children(&mut parent_node.walk())
                .any(|c| c.kind() == "=" || c.kind() == "by")
        {
            indent += ctx.options.continuation_indent;
        }
        curr = curr.and_then(|curr_node| curr_node.parent());
    }
    indent
}

fn collapsed_statement_len<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Option<usize> {
    if node.kind() == "property_declaration" {
        let mut len = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let child_kind = child.kind();
            if child_kind == "modifiers" {
                let mut mod_cursor = child.walk();
                let mut mod_len = 0;
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() != "annotation" {
                        if mod_child.start_position().row != mod_child.end_position().row {
                            return None;
                        }
                        mod_len += mod_child.end_byte().saturating_sub(mod_child.start_byte()) + 1;
                    }
                }
                len += mod_len;
            } else if child_kind == "multi_variable_declaration" {
                if child.start_position().row != child.end_position().row {
                    return None;
                }
                len += child.end_byte().saturating_sub(child.start_byte()) + 1;
            } else if matches!(
                child_kind,
                "val" | "var" | "binding_pattern_kind" | "variable_declaration"
            ) {
                len += child.end_byte().saturating_sub(child.start_byte()) + 1;
            } else if child_kind == "=" {
                len += 2;
            } else if matches!(child_kind, "comment" | "line_comment" | "multiline_comment") {
                continue;
            } else if child.start_position().row == child.end_position().row {
                len += child.end_byte().saturating_sub(child.start_byte());
            } else {
                return None;
            }
        }
        if len > 0 { Some(len) } else { None }
    } else if node.kind() == "assignment" {
        let mut cursor = node.walk();
        let mut len = 0;
        for child in node.children(&mut cursor) {
            let child_kind = child.kind();
            if matches!(child_kind, "comment" | "line_comment" | "multiline_comment") {
                continue;
            } else if matches!(child_kind, "=" | "+=" | "-=" | "*=" | "/=" | "%=") {
                len += child_kind.len() + 2;
            } else if child.start_position().row == child.end_position().row {
                len += child.end_byte().saturating_sub(child.start_byte());
            } else {
                return None;
            }
        }
        if len > 0 { Some(len) } else { None }
    } else if matches!(
        node.kind(),
        "call_expression" | "navigation_expression" | "jump_expression"
    ) {
        if node.start_position().row == node.end_position().row {
            Some(node.byte_range().len())
        } else {
            let text = ctx.node_text(&node);
            Some(crate::lower::expression_body::collapsed_text_len(text))
        }
    } else if node.kind() == "prefix_expression" {
        let child = node.child(node.child_count().saturating_sub(1))?;
        collapsed_statement_len(child, ctx)
    } else {
        None
    }
}

fn resolve_prev_navigation_expr<'a>(
    prev: Node<'a>,
    _ctx: &LowerContext<'a, '_>,
) -> (Option<Node<'a>>, usize) {
    if prev.kind() == "navigation_expression" {
        (Some(prev), 0)
    } else if prev.kind() == "call_expression"
        && let Some(callee) = prev.child(0)
        && callee.kind() == "navigation_expression"
    {
        let suffix_len = prev.end_byte().saturating_sub(callee.end_byte());
        (Some(callee), suffix_len)
    } else if matches!(prev.kind(), "getter" | "setter") {
        let nav = prev
            .children(&mut prev.walk())
            .find(|c| c.kind() == "function_body")
            .and_then(|b| {
                b.children(&mut b.walk()).find_map(|c| {
                    if c.kind() == "navigation_expression" {
                        Some((Some(c), 0))
                    } else if c.kind() == "call_expression"
                        && let Some(callee) = c.child(0)
                        && callee.kind() == "navigation_expression"
                    {
                        let suffix_len = c.end_byte().saturating_sub(callee.end_byte());
                        Some((Some(callee), suffix_len))
                    } else {
                        None
                    }
                })
            });
        nav.unwrap_or((None, 0))
    } else {
        (None, 0)
    }
}

fn resolve_comment_columns<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    is_standalone: bool,
    prefix: &str,
    override_col: Option<usize>,
) -> (usize, usize) {
    if let Some(col) = override_col {
        let block_indent = compute_comment_indent(node, ctx);
        return (col, block_indent);
    }
    if ctx.options.block_indent != 4 {
        let col = node.start_position().column;
        return (col, col);
    }
    let block_indent = compute_comment_indent(node, ctx);
    if is_standalone {
        return (block_indent, block_indent);
    }
    let col = if let Some(prev) = node.prev_sibling() {
        let (nav_expr, call_suffix_len) = resolve_prev_navigation_expr(prev, ctx);
        let comment_len = 1 + ctx.node_text(&node).len();
        if let Some(nav) = nav_expr
            && crate::lower::chain::is_chain_broken(
                &nav,
                ctx,
                crate::lower::expression_body::collapsed_text_len(ctx.node_text(&nav))
                    + comment_len,
            )
        {
            let last_elem_len = nav
                .children(&mut nav.walk())
                .last()
                .map(|c| ctx.node_text(&c).len())
                .unwrap_or(0);
            let extra = if matches!(prev.kind(), "getter" | "setter") {
                ctx.options.block_indent + 2 * ctx.options.continuation_indent
            } else {
                ctx.options.continuation_indent
            };
            block_indent + extra + last_elem_len + call_suffix_len + 1
        } else if let Some(flat_len) = collapsed_statement_len(prev, ctx) {
            block_indent + flat_len + 1
        } else {
            let trimmed_prefix = prefix.trim();
            if !trimmed_prefix.is_empty() {
                block_indent + trimmed_prefix.len() + 1
            } else {
                block_indent
            }
        }
    } else {
        let trimmed_prefix = prefix.trim();
        if !trimmed_prefix.is_empty() {
            block_indent + trimmed_prefix.len() + 1
        } else {
            block_indent
        }
    };
    (col, block_indent)
}

pub fn lower_comment<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    lower_comment_with_col(node, ctx, None)
}

pub fn lower_comment_with_col<'a>(
    node: Node<'a>,
    ctx: &LowerContext<'a, '_>,
    override_col: Option<usize>,
) -> Doc<'a> {
    let mut text = ctx.node_text(&node);
    if text.is_empty() {
        let end = node.start_byte();
        let prefix = ctx.source.get(..end).unwrap_or("");
        if let Some(start) = prefix.rfind("/*")
            && let Some(comment_end) = prefix.get(start..).and_then(|s| s.find("*/"))
        {
            text = prefix.get(start..start + comment_end + 2).unwrap_or("");
        }
    }
    if text.starts_with("//") {
        let line_start = ctx.source[..node.start_byte()]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = ctx.source.get(line_start..node.start_byte()).unwrap_or("");
        let is_standalone = prefix.trim().is_empty();

        let (start_col, base_indent) =
            resolve_comment_columns(node, ctx, is_standalone, prefix, override_col);
        let lines = normalize_line_comment(text, start_col, base_indent, ctx.options.max_width);
        if ctx.options.block_indent != 4 && lines.len() > 1 {
            let indent_str = " ".repeat(start_col);
            let mut combined = lines[0].to_string();
            for line in &lines[1..] {
                combined.push('\n');
                combined.push_str(&indent_str);
                combined.push_str(line);
            }
            Doc::Concat(vec![Doc::smol(combined), Doc::HardLine])
        } else {
            let mut docs = Vec::with_capacity(lines.len() * 2);
            for line in lines {
                docs.push(Doc::smol(line));
                docs.push(Doc::HardLine);
            }
            Doc::Concat(docs)
        }
    } else if text.starts_with("/**") {
        format_kdoc(text, ctx.options.max_width)
    } else {
        Doc::text(text)
    }
}
