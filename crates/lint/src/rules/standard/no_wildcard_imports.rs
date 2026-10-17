//! Built-in AST rule prohibiting wildcard / star imports (Ktlint rule 7).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that detects and prohibits wildcard imports.
pub struct NoWildcardImports;

impl AstRule for NoWildcardImports {
    fn id(&self) -> &'static str {
        "no-wildcard-imports"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["import_header"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "wildcard_import" {
                ctx.report(node, "Wildcard import is not allowed");
                break;
            }
        }
    }
}
