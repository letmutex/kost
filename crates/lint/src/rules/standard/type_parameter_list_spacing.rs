//! Built-in AST rule enforcing spacing in type parameter lists (Ktlint rule 39).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, spaces_after, spaces_before};

/// Rule that enforces proper spacing around and inside type parameter lists (`<T>`).
pub struct TypeParameterListSpacing;

impl AstRule for TypeParameterListSpacing {
    fn id(&self) -> &'static str {
        "type-parameter-list-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["type_parameters"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let child_count = node.child_count();
        if child_count < 2 {
            return;
        }

        let Some(left_angle) = node.child(0) else {
            return;
        };
        let Some(right_angle) = node.child(child_count - 1) else {
            return;
        };

        if left_angle.kind() != "<" || right_angle.kind() != ">" {
            return;
        }

        check_outer_spacing(node, left_angle, right_angle, source, ctx);
        check_inner_spacing(left_angle, right_angle, source, ctx);
    }
}

fn check_outer_spacing(
    node: tree_sitter::Node,
    left_angle: tree_sitter::Node,
    right_angle: tree_sitter::Node,
    source: &[u8],
    ctx: &mut LintCtx,
) {
    let Some(parent) = node.parent() else {
        return;
    };

    if matches!(
        parent.kind(),
        "function_declaration" | "property_declaration"
    ) {
        match spaces_before(left_angle, source) {
            WhitespaceBefore::Spaces(0) => {
                ctx.report(left_angle, "Missing spacing before '<'");
            }
            WhitespaceBefore::Spaces(count) if count > 1 => {
                ctx.report(left_angle, "Unexpected spacing before '<'");
            }
            _ => {}
        }

        match spaces_after(right_angle, source) {
            WhitespaceAfter::Spaces(0) => {
                ctx.report(right_angle, "Missing spacing after '>'");
            }
            WhitespaceAfter::Spaces(count) if count > 1 => {
                ctx.report(right_angle, "Unexpected spacing after '>'");
            }
            _ => {}
        }
    } else if matches!(parent.kind(), "class_declaration" | "type_alias")
        && let Some(prev) = node.prev_sibling()
        && prev.kind() == "type_identifier"
    {
        match spaces_before(left_angle, source) {
            WhitespaceBefore::Spaces(count) if count > 0 => {
                ctx.report(left_angle, "Unexpected spacing before '<'");
            }
            WhitespaceBefore::StartOfLine => {
                ctx.report(left_angle, "Unexpected spacing before '<'");
            }
            _ => {}
        }
    }
}

fn check_inner_spacing(
    left_angle: tree_sitter::Node,
    right_angle: tree_sitter::Node,
    source: &[u8],
    ctx: &mut LintCtx,
) {
    match spaces_after(left_angle, source) {
        WhitespaceAfter::Spaces(count) if count > 0 => {
            ctx.report(left_angle, "Unexpected spacing after '<'");
        }
        WhitespaceAfter::EndOfLine { trailing_spaces } if trailing_spaces > 0 => {
            ctx.report(left_angle, "Unexpected spacing after '<'");
        }
        _ => {}
    }

    match spaces_before(right_angle, source) {
        WhitespaceBefore::Spaces(count) if count > 0 => {
            ctx.report(right_angle, "Unexpected spacing before '>'");
        }
        _ => {}
    }
}
