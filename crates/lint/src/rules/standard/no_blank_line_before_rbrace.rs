//! Built-in AST rule forbidding blank lines before closing curly brace (Ktlint rule 67).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that forbids blank lines immediately preceding a closing curly brace `}`.
pub struct NoBlankLineBeforeRbrace;

impl AstRule for NoBlankLineBeforeRbrace {
    fn id(&self) -> &'static str {
        "no-blank-line-before-rbrace"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["}"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(prev_sibling) = ctx.prev_sibling() else {
            return;
        };

        let brace_row = node.start_position().row;
        let prev_end_row = prev_sibling.end_position().row;

        if brace_row > prev_end_row + 1 {
            ctx.report(node, "Unexpected blank line(s) before \"}\"");
        }
    }
}
