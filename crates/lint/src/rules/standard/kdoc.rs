//! Built-in AST rule validating KDoc comments and placement (Ktlint rule 94).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::is_kdoc;

/// Rule that validates KDoc placement on declarations.
pub struct Kdoc;

impl AstRule for Kdoc {
    fn id(&self) -> &'static str {
        "kdoc"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["multiline_comment", "comment"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let Ok(text) = node.utf8_text(source) else {
            return;
        };

        if !is_kdoc(text) {
            return;
        }

        check_kdoc_placement(node, ctx);
    }
}

fn check_kdoc_placement(node: tree_sitter::Node, ctx: &mut LintCtx) {
    if is_inside_disallowed_block(node) {
        ctx.report(
            node,
            "A KDoc is not allowed inside a block or function body",
        );
        return;
    }

    if let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "primary_constructor" | "class_parameters" | "function_value_parameters"
        ) {
            let mut next = node.next_named_sibling();
            while let Some(sibling) = next {
                if matches!(sibling.kind(), "class_parameter" | "parameter") {
                    return;
                }
                next = sibling.next_named_sibling();
            }
            ctx.report(node, "A KDoc is allowed only at start of declaration");
            return;
        } else if is_allowed_declaration(parent.kind()) && node.prev_named_sibling().is_some() {
            ctx.report(node, "A KDoc is allowed only at start of declaration");
            return;
        }
    }

    if !has_following_declaration(node) {
        ctx.report(node, "A dangling toplevel KDoc is not allowed");
    }
}

fn is_inside_disallowed_block(node: tree_sitter::Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "class_body" | "object_literal") {
            return false;
        }
        if matches!(
            parent.kind(),
            "statements"
                | "block"
                | "function_body"
                | "value_arguments"
                | "value_argument"
                | "type_arguments"
                | "lambda_literal"
                | "when_expression"
                | "if_expression"
        ) {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn has_following_declaration(node: tree_sitter::Node) -> bool {
    let mut current = node;
    loop {
        let mut next = current.next_sibling();
        while let Some(sibling) = next {
            if is_allowed_declaration(sibling.kind()) {
                return true;
            }
            if sibling.is_named()
                && !matches!(
                    sibling.kind(),
                    "comment" | "multiline_comment" | "line_comment"
                )
            {
                return false;
            }
            next = sibling.next_sibling();
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent.kind() == "source_file" || parent.kind() == "class_body" {
            break;
        }
        current = parent;
    }
    false
}

fn is_allowed_declaration(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration"
            | "object_declaration"
            | "companion_object"
            | "function_declaration"
            | "property_declaration"
            | "primary_constructor"
            | "secondary_constructor"
            | "type_alias"
            | "enum_entry"
            | "parameter"
            | "class_parameter"
    )
}
