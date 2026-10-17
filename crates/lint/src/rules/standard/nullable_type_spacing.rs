//! Built-in AST rule enforcing zero spacing before the `?` nullability specifier (Ktlint rule 34).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, spaces_before};

/// Rule that enforces zero spaces between a type and its `?` nullability specifier.
pub struct NullableTypeSpacing;

impl AstRule for NullableTypeSpacing {
    fn id(&self) -> &'static str {
        "nullable-type-spacing"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["nullable_type"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "?" | "quest") {
                match spaces_before(child, source) {
                    WhitespaceBefore::Spaces(count) if count > 0 => {
                        ctx.report(child, "Unexpected spacing before '?'");
                    }
                    WhitespaceBefore::StartOfLine => {
                        ctx.report(child, "Unexpected spacing before '?'");
                    }
                    _ => {}
                }
                break;
            }
        }
    }
}
