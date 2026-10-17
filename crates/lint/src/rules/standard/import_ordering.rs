//! Built-in AST rule enforcing canonical import ordering and duplicate detection (Ktlint rule 6).

use ahash::AHashSet;
use smol_str::SmolStr;

use crate::context::LintCtx;
use crate::rule::AstRule;

/// Rule that enforces canonical import ordering and prohibits duplicate imports.
pub struct ImportOrdering;

struct OrderedImportInfo {
    path: SmolStr,
    alias: Option<SmolStr>,
    is_wildcard: bool,
}

impl AstRule for ImportOrdering {
    fn id(&self) -> &'static str {
        "import-ordering"
    }

    fn interested_kinds(&self) -> &'static [&'static str] {
        &["import_list"]
    }

    fn check_node(&self, node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
        let mut imports = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "import_header" {
                let info = extract_import_info(child, source);
                imports.push((child, info));
            }
        }

        if imports.len() <= 1 {
            return;
        }

        let mut seen_imports = AHashSet::new();
        let mut previous_rank: Option<(u8, &str, Option<&str>)> = None;

        for (import_node, info) in &imports {
            let import_key = (info.path.as_str(), info.is_wildcard, info.alias.as_deref());
            if !seen_imports.insert(import_key) {
                let message = if info.is_wildcard {
                    crate::concat_str!("Duplicate import: ", &info.path, ".*")
                } else if let Some(alias) = &info.alias {
                    crate::concat_str!("Duplicate import: ", &info.path, " as ", alias)
                } else {
                    crate::concat_str!("Duplicate import: ", &info.path)
                };
                ctx.report(*import_node, message);
                continue;
            }

            let group = classify_import_group(&info.path, info.alias.is_some());
            let current_rank = (group, info.path.as_str(), info.alias.as_deref());

            if let Some(prev) = previous_rank
                && current_rank < prev
            {
                ctx.report(*import_node, "Imports are not correctly ordered");
            }

            previous_rank = Some(current_rank);
        }
    }
}

fn extract_import_info(node: tree_sitter::Node, source: &[u8]) -> OrderedImportInfo {
    let mut path = SmolStr::default();
    let mut alias = None;
    let mut is_wildcard = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            if let Ok(text) = child.utf8_text(source) {
                path = SmolStr::new(text);
            }
        } else if child.kind() == "wildcard_import" {
            is_wildcard = true;
        } else if child.kind() == "import_alias" {
            let mut alias_cursor = child.walk();
            for alias_child in child.children(&mut alias_cursor) {
                if matches!(alias_child.kind(), "simple_identifier" | "type_identifier")
                    && let Ok(name) = alias_child.utf8_text(source)
                {
                    alias = Some(SmolStr::new(name));
                    break;
                }
            }
        }
    }

    OrderedImportInfo {
        path,
        alias,
        is_wildcard,
    }
}

fn classify_import_group(path: &str, is_alias: bool) -> u8 {
    if is_alias {
        3
    } else if path.starts_with("java.") || path.starts_with("javax.") {
        1
    } else if path.starts_with("kotlin.") {
        2
    } else {
        0
    }
}
