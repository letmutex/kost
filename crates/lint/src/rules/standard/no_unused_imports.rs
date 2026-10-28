//! Built-in AST rule detecting unused imports (Ktlint rule 8).

use ahash::AHashSet;

use crate::context::LintCtx;
use crate::rule::AstRule;
use crate::util::node_kind;

/// Rule that detects unused import statements.
pub struct NoUnusedImports;

struct ImportInfo<'a> {
    symbol: &'a str,
    full_path: &'a str,
}

impl AstRule for NoUnusedImports {
    fn id(&self) -> &'static str {
        "no-unused-imports"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn check_node(&self, _node: tree_sitter::Node, _source: &[u8], _ctx: &mut LintCtx) {}

    fn has_after_node(&self) -> bool {
        true
    }

    fn ignores_suppressions(&self) -> bool {
        true
    }

    fn after_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut imports = Vec::new();
        let mut unreferenced: AHashSet<&str> = AHashSet::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if node_kind(child) == "import_list" {
                let mut list_cursor = child.walk();
                for inner_child in child.children(&mut list_cursor) {
                    if node_kind(inner_child) == "import_header"
                        && let Some(info) = parse_import_target(inner_child, source)
                    {
                        if !is_operator_or_component(info.symbol) {
                            unreferenced.insert(info.symbol);
                        }
                        imports.push((inner_child, info));
                    }
                }
            }
        }

        if unreferenced.is_empty() {
            return;
        }

        scan_used_identifiers_with_cursor(node, source, &mut unreferenced);

        for (import_node, info) in imports {
            if unreferenced.contains(info.symbol) {
                let message = common::concat_str!("Unused import: ", info.full_path);
                ctx.report(import_node, message);
            }
        }
    }
}

fn parse_import_target<'a>(node: tree_sitter::Node, source: &'a [u8]) -> Option<ImportInfo<'a>> {
    let mut is_wildcard = false;
    let mut full_path = "";
    let mut alias = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = node_kind(child);
        if kind == "wildcard_import" {
            is_wildcard = true;
        } else if kind == "identifier" {
            if let Ok(text) = child.utf8_text(source) {
                full_path = text;
            }
        } else if kind == "import_alias" {
            let mut alias_cursor = child.walk();
            for alias_child in child.children(&mut alias_cursor) {
                if matches!(
                    node_kind(alias_child),
                    "simple_identifier" | "type_identifier"
                ) && let Ok(name) = alias_child.utf8_text(source)
                {
                    alias = Some(name.trim_matches('`'));
                    break;
                }
            }
        }
    }

    if is_wildcard || full_path.is_empty() {
        return None;
    }

    let symbol = match alias {
        Some(alias_name) => alias_name,
        None => {
            let last_segment = full_path.rsplit('.').next().unwrap_or(full_path);
            last_segment.trim_matches('`')
        }
    };

    Some(ImportInfo { symbol, full_path })
}

fn scan_used_identifiers_with_cursor(
    root: tree_sitter::Node,
    source: &[u8],
    unreferenced: &mut AHashSet<&str>,
) {
    let mut cursor = root.walk();
    let mut import_depth: usize = 0;
    loop {
        if unreferenced.is_empty() {
            return;
        }
        let curr = cursor.node();
        let kind = node_kind(curr);
        let is_import_node = kind == "import_header" || kind == "import_alias";
        let in_import = import_depth > 0 || is_import_node;
        let skip_children = kind == "package_header" || kind == "import_alias";

        if !in_import
            && matches!(
                kind,
                "simple_identifier" | "type_identifier" | "interpolated_identifier"
            )
        {
            if let Ok(text) = curr.utf8_text(source) {
                unreferenced.remove(text.trim_matches('`'));
            }
        } else if kind == "multiline_comment"
            && let Ok(comment_text) = curr.utf8_text(source)
            && comment_text.starts_with("/**")
        {
            extract_kdoc_symbols(comment_text, unreferenced);
        }

        if !skip_children && cursor.goto_first_child() {
            if in_import {
                import_depth += 1;
            }
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
            import_depth = import_depth.saturating_sub(1);
        }
    }
}

fn extract_kdoc_symbols(kdoc: &str, unreferenced: &mut AHashSet<&str>) {
    for (start_idx, c) in kdoc.char_indices() {
        if c == '['
            && let Some(end_offset) = kdoc[start_idx..].find(']')
        {
            let inner = kdoc[start_idx + 1..start_idx + end_offset].trim();
            for segment in inner.split(|ch: char| !ch.is_alphanumeric() && ch != '_') {
                if !segment.is_empty() {
                    unreferenced.remove(segment);
                }
            }
        }
    }

    for line in kdoc.lines() {
        let trimmed = line.trim_start_matches(|c: char| c == '*' || c == '/' || c.is_whitespace());
        if let Some(rest) = trimmed
            .strip_prefix("@see ")
            .or_else(|| trimmed.strip_prefix("@throws "))
            .or_else(|| trimmed.strip_prefix("@exception "))
        {
            let first_word = rest.split_whitespace().next().unwrap_or("");
            for segment in first_word.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if !segment.is_empty() {
                    unreferenced.remove(segment);
                }
            }
        }
    }
}

fn is_operator_or_component(name: &str) -> bool {
    const OPERATORS: &[&str] = &[
        "assign",
        "plus",
        "minus",
        "times",
        "div",
        "rem",
        "plusAssign",
        "minusAssign",
        "timesAssign",
        "divAssign",
        "remAssign",
        "get",
        "set",
        "invoke",
        "contains",
        "iterator",
        "rangeTo",
        "rangeUntil",
        "getValue",
        "setValue",
        "provideDelegate",
        "compareTo",
        "equals",
        "inc",
        "dec",
        "unaryPlus",
        "unaryMinus",
        "not",
    ];

    if OPERATORS.contains(&name) {
        return true;
    }

    if let Some(suffix) = name.strip_prefix("component")
        && !suffix.is_empty()
        && suffix.chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }

    false
}
