//! Built-in AST rule enforcing trailing comma at call sites (Ktlint rule 77).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces or forbids trailing commas on call-site argument lists and type arguments.
pub struct TrailingCommaOnCallSite;

impl AstRule for TrailingCommaOnCallSite {
    fn id(&self) -> &'static str {
        "trailing-comma-on-call-site"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["value_arguments", "type_arguments"]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        let mut last_item = None;
        let mut comma_after_last = None;
        let mut closing_delim = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "value_argument" || kind == "type_projection" {
                last_item = Some(child);
                comma_after_last = None;
            } else if kind == "," && last_item.is_some() {
                comma_after_last = Some(child);
            } else if (kind == ")" || kind == ">") && last_item.is_some() {
                closing_delim = Some(child);
            }
        }

        let (Some(last_item_node), Some(delimiter)) = (last_item, closing_delim) else {
            return;
        };

        let is_multiline = delimiter.start_position().row > last_item_node.end_position().row;
        let allow_trailing = ctx
            .editorconfig
            .map(|ec| ec.allow_trailing_comma_on_call_site())
            .unwrap_or(true);

        if !allow_trailing {
            if let Some(comma_node) = comma_after_last {
                ctx.report(comma_node, "Unnecessary trailing comma");
            }
            return;
        }

        if is_multiline && comma_after_last.is_none() {
            ctx.report(last_item_node, "Missing trailing comma");
        } else if !is_multiline && let Some(comma_node) = comma_after_last {
            ctx.report(comma_node, "Unnecessary trailing comma");
        }
    }
}
