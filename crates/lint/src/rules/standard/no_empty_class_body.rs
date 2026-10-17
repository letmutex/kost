//! Built-in AST rule forbidding empty class bodies (Ktlint rule 71).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that flags unnecessary empty braces in classes, interfaces, and objects.
pub struct NoEmptyClassBody;

impl AstRule for NoEmptyClassBody {
    fn id(&self) -> &'static str {
        "no-empty-class-body"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["class_body"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(parent) = ctx.parent() else {
            return;
        };

        if parent.kind() == "object_literal" || is_enum_declaration(parent) {
            return;
        }

        if is_empty_body(node) {
            ctx.report(node, "Unnecessary block (\"{}\")");
        }
    }
}

fn is_enum_declaration(parent: tree_sitter::Node) -> bool {
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut modifier_cursor = child.walk();
            for modifier in child.children(&mut modifier_cursor) {
                if modifier.kind() == "class_modifier" {
                    let mut class_mod_cursor = modifier.walk();
                    for keyword in modifier.children(&mut class_mod_cursor) {
                        if keyword.kind() == "enum" {
                            return true;
                        }
                    }
                } else if modifier.kind() == "enum" {
                    return true;
                }
            }
        }
    }
    false
}

fn is_empty_body(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "{" || kind == "}" || kind == ";" {
            continue;
        }
        if child.is_named() || matches!(kind, "line_comment" | "multiline_comment" | "comment") {
            return false;
        }
    }
    true
}
