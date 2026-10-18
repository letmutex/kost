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
        let mut previous_rank: Option<(usize, &str, Option<&str>)> = None;
        let layout_raw = ctx
            .editorconfig
            .and_then(|c| {
                c.raw_properties
                    .get("ij_kotlin_imports_layout")
                    .or_else(|| c.raw_properties.get("kotlin_imports_layout"))
            })
            .map(|s| s.as_str());
        let layout = ImportLayout::from_raw(layout_raw);

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

            let group = classify_import_group(&info.path, info.alias.is_some(), &layout);
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

enum ImportLayout<'a> {
    Ascii,
    Idea,
    Custom(smallvec::SmallVec<[&'a str; 8]>),
}

impl<'a> ImportLayout<'a> {
    fn from_raw(raw: Option<&'a str>) -> Self {
        let layout_str = raw.unwrap_or("*,java.**,javax.**,kotlin.**,^");
        if layout_str == "*" || layout_str == "ascii" {
            Self::Ascii
        } else if layout_str == "*,java.**,javax.**,kotlin.**,^" || layout_str == "idea" {
            Self::Idea
        } else {
            let patterns = layout_str
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty() && *p != "|")
                .collect();
            Self::Custom(patterns)
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

fn classify_import_group(path: &str, is_alias: bool, layout: &ImportLayout) -> usize {
    match layout {
        ImportLayout::Ascii => 0,
        ImportLayout::Idea => {
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
        ImportLayout::Custom(patterns) => match_custom_layout(path, is_alias, patterns),
    }
}

fn match_custom_layout(path: &str, is_alias: bool, patterns: &[&str]) -> usize {
    let mut best_index = usize::MAX;
    let mut best_len = 0;
    let mut all_other_alias_index = None;
    let mut all_other_index = None;

    for (index, &pattern) in patterns.iter().enumerate() {
        if pattern == "^" {
            all_other_alias_index = Some(index);
            continue;
        }
        if pattern == "*" {
            all_other_index = Some(index);
            continue;
        }

        let (pat_without_alias, pat_has_alias) = if let Some(stripped) = pattern.strip_prefix('^') {
            (stripped, true)
        } else {
            (pattern, false)
        };

        if is_alias != pat_has_alias {
            continue;
        }

        let (prefix, is_recursive, is_wildcard) =
            if let Some(p) = pat_without_alias.strip_suffix(".**") {
                (p, true, true)
            } else if let Some(p) = pat_without_alias.strip_suffix(".*") {
                (p, false, true)
            } else {
                (pat_without_alias, false, false)
            };

        let matches = if let Some(rest) = path.strip_prefix(prefix) {
            if is_recursive {
                rest.is_empty() || rest.starts_with('.')
            } else if is_wildcard {
                rest.starts_with('.') && !rest[1..].contains('.')
            } else {
                rest.is_empty()
            }
        } else {
            false
        };

        if matches && prefix.len() > best_len {
            best_len = prefix.len();
            best_index = index;
        }
    }

    if best_index != usize::MAX {
        return best_index;
    }

    if is_alias {
        if let Some(idx) = all_other_alias_index.or(all_other_index) {
            return idx;
        }
    } else if let Some(idx) = all_other_index {
        return idx;
    }

    patterns.len()
}
