//! Built-in AST rule enforcing Kotlin package name conventions (Ktlint rule 2).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces lowercase package names without underscores or uppercase characters.
pub struct PackageName;

impl AstRule for PackageName {
    fn id(&self) -> &'static str {
        "package-name"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["package_header"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                check_package_identifier(child, source, ctx);
            }
        }
    }
}

fn check_package_identifier(identifier_node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let mut cursor = identifier_node.walk();
    for child in identifier_node.children(&mut cursor) {
        if matches!(child.kind(), "simple_identifier" | "type_identifier") {
            let Ok(raw_segment) = child.utf8_text(source) else {
                continue;
            };

            let segment = if raw_segment.starts_with('`')
                && raw_segment.ends_with('`')
                && raw_segment.len() >= 2
            {
                &raw_segment[1..raw_segment.len() - 1]
            } else {
                raw_segment
            };

            if segment.chars().any(|c| c.is_ascii_uppercase()) {
                ctx.report(child, "Package name must not contain uppercase characters");
            } else if segment.contains('_') {
                ctx.report(child, "Package name must not contain underscores");
            }
        }
    }
}
