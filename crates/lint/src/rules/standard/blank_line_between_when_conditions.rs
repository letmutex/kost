//! Built-in AST rule enforcing blank lines between when conditions (Ktlint rule 65).

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::has_blank_line_between_decls;

/// Rule that enforces blank lines between `when` entries when any entry is multiline.
pub struct BlankLineBetweenWhenConditions;

impl AstRule for BlankLineBetweenWhenConditions {
    fn id(&self) -> &'static str {
        "blank-line-between-when-conditions"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["when_expression"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut cursor = node.walk();
        let mut when_entries = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "when_entry" {
                when_entries.push(child);
            }
        }

        if when_entries.len() <= 1 {
            return;
        }

        let has_multiline_entry = when_entries
            .iter()
            .any(|entry| entry.start_position().row != entry.end_position().row);

        if !has_multiline_entry {
            return;
        }

        for i in 1..when_entries.len() {
            let prev_entry = when_entries[i - 1];
            let curr_entry = when_entries[i];

            if !has_blank_line_between_decls(prev_entry, curr_entry) {
                ctx.report(curr_entry, "Expected a blank line between when conditions");
            }
        }
    }
}
