//! Built-in AST rule enforcing property wrapping (Ktlint rule 62).

use super::helpers::{next_code_sibling, prev_code_sibling};
use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces line wrapping for property accessors and components.
pub struct PropertyWrapping;

impl AstRule for PropertyWrapping {
    fn id(&self) -> &'static str {
        "property-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["property_declaration", "getter", "setter"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        match node.kind() {
            "property_declaration" => {
                check_property_declaration(node, ctx);
            }
            "getter" | "setter" => {
                check_standalone_accessor(node, ctx);
            }
            _ => {}
        }
    }
}

fn check_property_declaration(node: tree_sitter::Node, ctx: &mut LintCtx) {
    let mut cursor = node.walk();
    let mut accessors = Vec::new();
    let mut header_end_row = node.start_position().row;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "getter" | "setter" => {
                accessors.push(child);
            }
            _ => {
                if child.is_named() && accessors.is_empty() {
                    header_end_row = header_end_row.max(child.end_position().row);
                }
            }
        }
    }

    if accessors.is_empty() {
        return;
    }

    let has_sibling_accessor = next_code_sibling(node)
        .is_some_and(|sibling| sibling.kind() == "getter" || sibling.kind() == "setter");

    let decl_keyword = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "binding_pattern_kind" || c.kind() == "val" || c.kind() == "var");
    let decl_start_row = decl_keyword
        .map(|k| k.start_position().row)
        .unwrap_or_else(|| node.start_position().row);

    let is_multiline_decl = decl_start_row != header_end_row;
    let requires_wrapping = accessors.len() > 1
        || has_sibling_accessor
        || is_multiline_decl
        || is_braced_accessor(accessors[0])
        || accessors[0].start_position().row != accessors[0].end_position().row;

    if requires_wrapping {
        let first = accessors[0];
        if first.start_position().row <= header_end_row {
            ctx.report(first, "Getter or setter should be on a new line");
        }
        for i in 1..accessors.len() {
            let prev = accessors[i - 1];
            let curr = accessors[i];
            if curr.start_position().row <= prev.end_position().row {
                ctx.report(curr, "Getter or setter should be on a new line");
            }
        }
    }
}

fn check_standalone_accessor(node: tree_sitter::Node, ctx: &mut LintCtx) {
    if node
        .parent()
        .is_some_and(|p| p.kind() == "property_declaration")
    {
        return;
    }

    if let Some(prev) = prev_code_sibling(node)
        && matches!(prev.kind(), "property_declaration" | "getter" | "setter")
        && node.start_position().row <= prev.end_position().row
    {
        ctx.report(node, "Getter or setter should be on a new line");
    }
}

fn is_braced_accessor(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "{" {
            return true;
        }
        if child.kind() == "function_body" {
            let mut body_cursor = child.walk();
            for body_child in child.children(&mut body_cursor) {
                if body_child.kind() == "{" || body_child.kind() == "statements" {
                    return true;
                }
            }
        }
    }
    false
}
