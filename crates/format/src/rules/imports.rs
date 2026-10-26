use ahash::AHashSet;
use tree_sitter::Node;

/// Standard Kotlin operator names that must never be pruned as unused imports.
pub const OPERATOR_NAMES: &[&str] = &[
    "unaryPlus",
    "unaryMinus",
    "not",
    "inc",
    "dec",
    "plus",
    "minus",
    "times",
    "div",
    "rem",
    "mod",
    "rangeTo",
    "rangeUntil",
    "contains",
    "get",
    "set",
    "invoke",
    "plusAssign",
    "minusAssign",
    "timesAssign",
    "divAssign",
    "remAssign",
    "modAssign",
    "equals",
    "compareTo",
    "iterator",
    "next",
    "hasNext",
    "and",
    "or",
    "getValue",
    "setValue",
    "provideDelegate",
    "assign",
];

#[inline]
pub fn is_operator_name(name: &str) -> bool {
    if OPERATOR_NAMES.contains(&name) {
        return true;
    }
    if let Some(rest) = name.strip_prefix("component")
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub raw_text: String,
    pub full_path: String,
    pub short_name: String,
    pub alias: Option<String>,
    pub is_wildcard: bool,
    pub is_used: bool,
    pub is_from_same_package: bool,
    pub trailing_comment: Option<String>,
}

impl ImportEntry {
    pub fn canonical_key(&self) -> String {
        let clean_path = self.full_path.replace('`', "");
        let clean_alias = match &self.alias {
            Some(a) => common::concat_str!("as ", &a.replace('`', "")),
            None => "null".to_string(),
        };
        let wildcard_part = if self.is_wildcard { "*" } else { "" };
        common::concat_str!(&clean_path, " ", &clean_alias, " ", wildcard_part)
    }
}

pub fn clean_identifier_path(node: Node<'_>, source: &str) -> String {
    let mut segments = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "simple_identifier" | "type_identifier")
            && let Some(s) = source.get(child.start_byte()..child.end_byte())
        {
            segments.push(s.trim());
        }
    }
    if segments.is_empty()
        && let Some(s) = source.get(node.start_byte()..node.end_byte())
    {
        return s.split('.').map(str::trim).collect::<Vec<_>>().join(".");
    }
    segments.join(".")
}

pub fn collect_used_references<'a>(root: Node<'_>, source: &'a str) -> AHashSet<&'a str> {
    let mut used = AHashSet::with_capacity(256);
    extract_all_kdoc_references(source, &mut used);
    collect_refs_recursive(root, source, &mut used);
    used
}

fn extract_all_kdoc_references<'a>(source: &'a str, used: &mut AHashSet<&'a str>) {
    if !source.contains("/**") {
        return;
    }
    let mut search_idx = 0;
    while let Some(start) = source[search_idx..].find("/**") {
        let abs_start = search_idx + start;
        let Some(end) = source[abs_start..].find("*/") else {
            break;
        };
        let comment = &source[abs_start..abs_start + end + 2];
        extract_kdoc_from_text(comment, used);
        search_idx = abs_start + end + 2;
    }
}

fn collect_refs_recursive<'a>(node: Node<'_>, source: &'a str, used: &mut AHashSet<&'a str>) {
    let kind = node.kind();
    if kind == "package_header" || kind == "import_list" || kind == "import_header" {
        return;
    }

    if kind == "simple_identifier" || kind == "type_identifier" || kind == "identifier" {
        if let Some(slice) = source.get(node.start_byte()..node.end_byte()) {
            let clean = slice.trim().trim_matches('`');
            if !clean.is_empty() && !clean.contains('.') {
                used.insert(clean);
            }
        }
        return;
    }

    if node.child_count() == 0 {
        if let Some(slice) = source.get(node.start_byte()..node.end_byte()) {
            let trimmed = slice.trim();
            if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() >= 2 {
                let clean = trimmed.trim_matches('`');
                if !clean.is_empty() {
                    used.insert(clean);
                }
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_refs_recursive(child, source, used);
    }
}

fn extract_kdoc_from_text<'a>(text: &'a str, used: &mut AHashSet<&'a str>) {
    let mut search_idx = 0;
    while let Some(start_bracket) = text[search_idx..].find('[') {
        let actual_start = search_idx + start_bracket + 1;
        if let Some(end_bracket) = text[actual_start..].find(']') {
            let link = &text[actual_start..actual_start + end_bracket];
            for segment in link.trim().trim_matches('`').split('.') {
                let clean = segment.trim().trim_matches('`');
                if !clean.is_empty() {
                    used.insert(clean);
                }
            }
            search_idx = actual_start + end_bracket + 1;
        } else {
            break;
        }
    }

    if text.contains('@') {
        for line in text.lines() {
            let trimmed = line.trim().trim_start_matches('*').trim();
            for tag in ["@see", "@throws", "@exception", "@sample"] {
                if let Some(rest) = trimmed.strip_prefix(tag) {
                    let rest = rest.trim();
                    let first_word = rest.split_whitespace().next().unwrap_or("");
                    let clean_word = first_word.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != '_' && c != '.' && c != '#'
                    });
                    for segment in clean_word.split(['.', '#']) {
                        if !segment.is_empty() {
                            used.insert(segment);
                        }
                    }
                }
            }
        }
    }
}

fn extract_import_details(node: Node<'_>, source: &str) -> (String, Option<String>, bool) {
    let mut full_path = String::new();
    let mut alias = None;
    let mut is_wildcard = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                full_path = clean_identifier_path(child, source);
            }
            "wildcard_import" => {
                is_wildcard = true;
            }
            "import_alias" => {
                let mut alias_cursor = child.walk();
                for ac in child.children(&mut alias_cursor) {
                    if ac.kind() != "as"
                        && let Some(s) = source.get(ac.start_byte()..ac.end_byte())
                    {
                        let s = s.trim();
                        if !s.is_empty() {
                            alias = Some(s.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (full_path, alias, is_wildcard)
}

fn derive_short_name(full_path: &str, alias: Option<&str>, is_wildcard: bool) -> String {
    if let Some(al) = alias {
        al.trim_matches('`').to_string()
    } else if is_wildcard {
        "*".to_string()
    } else {
        let clean = full_path.trim_matches('`');
        clean
            .rsplit('.')
            .next()
            .unwrap_or(clean)
            .trim_matches('`')
            .to_string()
    }
}

fn check_same_package(full_path: &str, package_name: Option<&str>) -> bool {
    let Some(pkg) = package_name else {
        return false;
    };
    if full_path.starts_with('`') {
        return false;
    }
    if let Some((parent, _)) = full_path.rsplit_once('.') {
        parent == pkg
    } else {
        false
    }
}

pub fn parse_import_header(
    node: Node<'_>,
    source: &str,
    package_name: Option<&str>,
    used_references: &AHashSet<&str>,
) -> Option<ImportEntry> {
    let raw_text = source
        .get(node.start_byte()..node.end_byte())?
        .trim()
        .to_string();
    let (mut full_path, alias, is_wildcard) = extract_import_details(node, source);

    if full_path.is_empty() && !raw_text.is_empty() {
        let without_import = raw_text.strip_prefix("import").unwrap_or(&raw_text).trim();
        full_path = without_import
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
    }

    let short_name = derive_short_name(&full_path, alias.as_deref(), is_wildcard);
    let is_used = is_wildcard
        || is_operator_name(&short_name)
        || used_references.contains(short_name.as_str());
    let is_from_same_package = check_same_package(&full_path, package_name);

    Some(ImportEntry {
        raw_text,
        full_path,
        short_name,
        alias,
        is_wildcard,
        is_used,
        is_from_same_package,
        trailing_comment: None,
    })
}
