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

fn collapsed_statement_len<'a>(node: Node<'a>) -> Option<usize> {
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
    } else if node.kind() == "jump_expression" {
        if node.start_position().row == node.end_position().row {
            Some(node.byte_range().len())
        } else {
            None
        }
    } else {
        None
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
    let col = if let Some(prev) = node.prev_sibling()
        && let Some(flat_len) = collapsed_statement_len(prev)
    {
        block_indent + flat_len + 1
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
