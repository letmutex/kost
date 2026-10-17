//! Built-in AST rule enforcing a blank line before file annotations (Ktlint rule 5).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::prev_meaningful_sibling;

/// Rule that enforces a blank line before file annotations when preceded by top-of-file comments.
pub struct BlankLineBeforeFileAnnotation;

impl AstRule for BlankLineBeforeFileAnnotation {
    fn id(&self) -> &'static str {
        "blank-line-before-file-annotation"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["file_annotation"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let Some(prev_node) = prev_meaningful_sibling(node) else {
            return;
        };

        // If the previous element is also a file_annotation, skip
        if prev_node.kind() == "file_annotation" {
            return;
        }

        if matches!(prev_node.kind(), "line_comment" | "multiline_comment") {
            let prev_end_row = prev_node.end_position().row;
            let annotation_start_row = node.start_position().row;

            if annotation_start_row <= prev_end_row + 1 {
                ctx.report(
                    node,
                    "Expected a blank line between top-of-file comment and file annotation",
                );
            }
        }
    }
}
