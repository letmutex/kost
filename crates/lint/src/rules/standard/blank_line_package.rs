//! Built-in AST rule enforcing a blank line before package declaration (Ktlint rule 3).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::prev_meaningful_sibling;

/// Rule that enforces a blank line before package declaration when preceded by comments or file annotations.
pub struct BlankLineBeforePackage;

impl AstRule for BlankLineBeforePackage {
    fn id(&self) -> &'static str {
        "blank-line-before-package"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["package_header"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(prev_node) = prev_meaningful_sibling(node) else {
            return;
        };

        let prev_end_row = prev_node.end_position().row;
        let package_start_row = node.start_position().row;

        if package_start_row <= prev_end_row + 1 {
            ctx.report(node, "Expected a blank line before package declaration");
        } else if package_start_row > prev_end_row + 2 {
            ctx.report(node, "Unexpected blank line(s) before package declaration");
        }
    }
}
