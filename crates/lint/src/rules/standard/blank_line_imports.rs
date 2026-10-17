//! Built-in AST rule enforcing a blank line before import list (Ktlint rule 4).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::prev_meaningful_sibling;

/// Rule that enforces a blank line before the import list when preceded by package or comments.
pub struct BlankLineBeforeImports;

impl AstRule for BlankLineBeforeImports {
    fn id(&self) -> &'static str {
        "blank-line-before-imports"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["import_list"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(prev_node) = prev_meaningful_sibling(node) else {
            return;
        };

        let prev_end_row = prev_node.end_position().row;
        let import_start_row = node.start_position().row;

        if import_start_row <= prev_end_row + 1 {
            ctx.report(node, "Expected a blank line before import list");
        } else if import_start_row > prev_end_row + 2 {
            ctx.report(node, "Unexpected blank line(s) before import list");
        }
    }
}
