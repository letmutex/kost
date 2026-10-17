//! Built-in AST rule enforcing enum entry wrapping (Ktlint rule 50).

use smallvec::SmallVec;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::{WhitespaceBefore, spaces_before};

/// Rule that enforces enum entries either all fit on a single line or each starts on a separate line.
pub struct EnumWrapping;

impl AstRule for EnumWrapping {
    fn id(&self) -> &'static str {
        "enum-wrapping"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["enum_class_body", "class_body"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut enum_entries: SmallVec<[tree_sitter::Node; 8]> = SmallVec::new();
        let mut closing_brace = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "enum_entry" => enum_entries.push(child),
                "}" => closing_brace = Some(child),
                _ => {}
            }
        }

        if enum_entries.is_empty() {
            return;
        }

        let is_multiline = node.start_position().row != node.end_position().row;

        if !is_multiline {
            return;
        }

        for entry in enum_entries {
            if spaces_before(entry, source) != WhitespaceBefore::StartOfLine {
                ctx.report(entry, "Enum entry should start on a separate line");
            }
        }

        if let Some(rbrace) = closing_brace
            && spaces_before(rbrace, source) != WhitespaceBefore::StartOfLine
        {
            ctx.report(rbrace, "Expected newline before '}'");
        }
    }
}
