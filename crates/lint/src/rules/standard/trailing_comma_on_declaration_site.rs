//! Built-in AST rule enforcing trailing comma at declaration sites (Ktlint rule 78).

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces or forbids trailing commas on declarations (parameters, types, enum entries).
pub struct TrailingCommaOnDeclarationSite;

impl AstRule for TrailingCommaOnDeclarationSite {
    fn id(&self) -> &'static str {
        "trailing-comma-on-declaration-site"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &[
            "function_value_parameters",
            "class_parameters",
            "primary_constructor",
            "type_parameters",
            "enum_class_body",
        ]
    }

    fn check_node(&self, node: tree_sitter::Node, _source: &[u8], ctx: &mut LintCtx) {
        if node.kind() == "enum_class_body" {
            check_enum_entries(node, ctx);
        } else {
            check_parameter_or_type_list(node, ctx);
        }
    }
}

fn check_parameter_or_type_list(node: tree_sitter::Node, ctx: &mut LintCtx) {
    let mut last_item = None;
    let mut comma_after_last = None;
    let mut closing_delimiter = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if matches!(kind, "parameter" | "class_parameter" | "type_parameter") {
            last_item = Some(child);
            comma_after_last = None;
        } else if kind == "," && last_item.is_some() {
            comma_after_last = Some(child);
        } else if (kind == ")" || kind == ">") && last_item.is_some() {
            closing_delimiter = Some(child);
        }
    }

    let (Some(last_item_node), Some(delimiter)) = (last_item, closing_delimiter) else {
        return;
    };

    let is_multiline = delimiter.start_position().row > last_item_node.end_position().row;
    let allow_trailing = ctx
        .editorconfig
        .map(|ec| ec.allow_trailing_comma_on_declaration_site())
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

fn check_enum_entries(node: tree_sitter::Node, ctx: &mut LintCtx) {
    let mut first_entry = None;
    let mut last_entry = None;
    let mut comma_after_last = None;
    let mut closing_delimiter = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "enum_entry" => {
                if first_entry.is_none() {
                    first_entry = Some(child);
                }
                last_entry = Some(child);
                comma_after_last = None;
            }
            "," if last_entry.is_some() => {
                comma_after_last = Some(child);
            }
            ";" | "}" if closing_delimiter.is_none() && last_entry.is_some() => {
                closing_delimiter = Some(child);
            }
            _ => {}
        }
    }

    let (Some(first), Some(last), Some(delimiter)) = (first_entry, last_entry, closing_delimiter)
    else {
        return;
    };

    let is_multiline = first.start_position().row != last.end_position().row
        || delimiter.start_position().row > last.end_position().row;
    let allow_trailing = ctx
        .editorconfig
        .map(|ec| ec.allow_trailing_comma_on_declaration_site())
        .unwrap_or(true);

    if !allow_trailing {
        if let Some(comma_node) = comma_after_last {
            ctx.report(comma_node, "Unnecessary trailing comma");
        }
        return;
    }

    if is_multiline && comma_after_last.is_none() {
        ctx.report(last, "Missing trailing comma");
    } else if !is_multiline && let Some(comma_node) = comma_after_last {
        ctx.report(comma_node, "Unnecessary trailing comma");
    }
}
