//! Built-in AST rule enforcing spacing in type argument lists (Ktlint rule 38).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceAfter, WhitespaceBefore, node_kind, spaces_after, spaces_before};

/// Rule that enforces proper spacing around and inside type argument lists (`<T>`).
pub struct TypeArgumentListSpacing;

impl AstRule for TypeArgumentListSpacing {
    fn id(&self) -> &'static str {
        "type-argument-list-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["type_arguments"]
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

        if node_kind(left_angle) != "<" || node_kind(right_angle) != ">" {
            return;
        }

        check_angle_brackets(node, left_angle, right_angle, source, ctx);
    }
}

fn check_angle_brackets(
    node: tree_sitter::Node,
    left_angle: tree_sitter::Node,
    right_angle: tree_sitter::Node,
    source: &[u8],
    ctx: &mut LintCtx,
) {
    let parent = ctx.parent();
    let is_call_suffix = parent.is_some_and(|p| node_kind(p) == "call_suffix");

    let should_check_before = if is_call_suffix {
        parent
            .and_then(|p| {
                ctx.ancestor(2)
                    .and_then(|gp| gp.child(0))
                    .filter(|&first| first != p)
            })
            .is_some()
    } else {
        ctx.prev_sibling()
            .is_some_and(|prev| matches!(node_kind(prev), "type_identifier" | "simple_identifier"))
    };

    if should_check_before {
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

    if is_call_suffix {
        let has_following_value_args = parent.is_some_and(|p| {
            let mut cursor = p.walk();
            p.children(&mut cursor)
                .any(|c| c.start_byte() >= node.end_byte() && node_kind(c) == "value_arguments")
        });
        if has_following_value_args
            && let WhitespaceAfter::Spaces(count) = spaces_after(right_angle, source)
            && count > 0
        {
            ctx.report(right_angle, "Unexpected spacing after '>'");
        }
    }
}
