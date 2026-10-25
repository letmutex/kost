//! Built-in AST rule prohibiting fully qualified type names.

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::types::TextRange;
use smallvec::SmallVec;

/// Rule that detects and prohibits fully qualified type names in type positions.
pub struct NoFullyQualifiedTypeNames;

impl AstRule for NoFullyQualifiedTypeNames {
    fn id(&self) -> &'static str {
        "no-fully-qualified-type-names"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["user_type"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        if ctx
            .ancestors
            .iter()
            .any(|ancestor| matches!(ancestor.kind(), "import_header" | "package_header"))
        {
            return;
        }

        let mut has_dot = false;
        let mut type_identifiers = SmallVec::<[tree_sitter::Node; 4]>::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "." {
                has_dot = true;
            } else if child.kind() == "type_identifier" {
                type_identifiers.push(child);
            }
        }

        if !has_dot || type_identifiers.len() < 2 {
            return;
        }

        let prefix_identifiers = &type_identifiers[..type_identifiers.len() - 1];
        let has_package_qualifier = prefix_identifiers.iter().any(|ident_node| {
            ident_node
                .utf8_text(source)
                .ok()
                .map(|raw_text| {
                    raw_text
                        .strip_prefix('`')
                        .and_then(|stripped| stripped.strip_suffix('`'))
                        .unwrap_or(raw_text)
                })
                .and_then(|clean_segment| clean_segment.chars().next())
                .is_some_and(|character| character.is_lowercase())
        });

        if !has_package_qualifier {
            return;
        }

        let mut clean_name = String::new();
        for ident_node in &type_identifiers {
            if let Ok(segment) = ident_node.utf8_text(source) {
                if !clean_name.is_empty() {
                    clean_name.push('.');
                }
                clean_name.push_str(segment);
            }
        }

        let first_node = type_identifiers[0];
        let last_node = type_identifiers[type_identifiers.len() - 1];
        let start_pos = first_node.start_position();
        let end_pos = last_node.end_position();
        let target_range = TextRange::new(
            (start_pos.row + 1) as u32,
            (start_pos.column + 1) as u32,
            (end_pos.row + 1) as u32,
            (end_pos.column + 1) as u32,
        );

        ctx.report_range(
            target_range,
            crate::concat_str!(
                "Fully qualified type name '",
                &clean_name,
                "' should not be used"
            ),
        );
    }
}
