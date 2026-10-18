//! Built-in AST rule detecting unused imports (Ktlint rule 8).

use ahash::AHashSet;
use smol_str::SmolStr;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that detects unused import statements.
pub struct NoUnusedImports;

struct ImportInfo {
    symbol: SmolStr,
    full_path: SmolStr,
}

impl AstRule for NoUnusedImports {
    fn id(&self) -> &'static str {
        "no-unused-imports"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["source_file"]
    }

    fn check_node(&self, _node: tree_sitter::Node, _source: &[u8], _ctx: &mut LintCtx) {}

    fn ignores_suppressions(&self) -> bool {
        true
    }

    fn after_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut imports = Vec::new();
        let mut unreferenced = AHashSet::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_list" {
                let mut list_cursor = child.walk();
                for inner_child in child.children(&mut list_cursor) {
                    if inner_child.kind() == "import_header"
                        && let Some(info) = parse_import_target(inner_child, source)
                    {
                        if !is_operator_or_component(&info.symbol) {
                            unreferenced.insert(info.symbol.clone());
                        }
                        imports.push((inner_child, info));
                    }
                }
            }
        }

        if unreferenced.is_empty() {
            return;
        }

        scan_used_identifiers(node, source, &mut unreferenced, false);

        for (import_node, info) in imports {
            if unreferenced.contains(&info.symbol) {
                let message = crate::concat_str!("Unused import: ", &info.full_path);
                ctx.report(import_node, message);
            }
        }
    }
}

fn parse_import_target(node: tree_sitter::Node, source: &[u8]) -> Option<ImportInfo> {
    let mut is_wildcard = false;
    let mut full_path = "";
    let mut alias = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "wildcard_import" {
            is_wildcard = true;
        } else if child.kind() == "identifier" {
            if let Ok(text) = child.utf8_text(source) {
                full_path = text;
            }
        } else if child.kind() == "import_alias" {
            let mut alias_cursor = child.walk();
            for alias_child in child.children(&mut alias_cursor) {
                if matches!(alias_child.kind(), "simple_identifier" | "type_identifier")
                    && let Ok(name) = alias_child.utf8_text(source)
                {
                    alias = Some(SmolStr::new(name.trim_matches('`')));
                    break;
                }
            }
        }
    }

    if is_wildcard || full_path.is_empty() {
        return None;
    }

    let symbol = match alias {
        Some(a) => a,
        None => {
            let last_segment = full_path.rsplit('.').next().unwrap_or(full_path);
            SmolStr::new(last_segment.trim_matches('`'))
        }
    };

    Some(ImportInfo {
        symbol,
        full_path: SmolStr::new(full_path),
    })
}

fn scan_used_identifiers(
    node: tree_sitter::Node,
    source: &[u8],
    unreferenced: &mut AHashSet<SmolStr>,
    in_import_header: bool,
) {
    if unreferenced.is_empty() {
        return;
    }

    let kind = node.kind();
    if kind == "package_header" {
        return;
    }

    let is_import = in_import_header || kind == "import_header" || kind == "import_alias";

    if matches!(
        kind,
        "simple_identifier" | "type_identifier" | "interpolated_identifier"
    ) {
        if !is_import && let Ok(text) = node.utf8_text(source) {
            unreferenced.remove(text.trim_matches('`'));
        }
    } else if kind == "multiline_comment"
        && let Ok(comment_text) = node.utf8_text(source)
        && comment_text.starts_with("/**")
    {
        extract_kdoc_symbols(comment_text, unreferenced);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        scan_used_identifiers(child, source, unreferenced, is_import);
        if unreferenced.is_empty() {
            break;
        }
    }
}

fn extract_kdoc_symbols(kdoc: &str, unreferenced: &mut AHashSet<SmolStr>) {
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
