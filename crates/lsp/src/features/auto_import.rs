use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{CompletionItem, Range, TextEdit, Url};
use tree_sitter::Node;

pub use super::auto_import_insertion::{
    collect_header_nodes, detect_newline, extract_import_path, find_ast_import_insertion,
    insert_after_header, insert_into_import_headers,
};
use crate::concat_str;
use crate::indexer::FileSymbols;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoImportData {
    pub target_uri: Url,
    pub package_name: String,
    pub symbol_name: String,
    pub container_name: Option<String>,
}

impl AutoImportData {
    pub fn new(
        target_uri: Url,
        package_name: impl Into<String>,
        symbol_name: impl Into<String>,
        container_name: Option<impl Into<String>>,
    ) -> Self {
        Self {
            target_uri,
            package_name: package_name.into(),
            symbol_name: symbol_name.into(),
            container_name: container_name.map(Into::into),
        }
    }

    pub fn full_name(&self) -> String {
        if let Some(ref container) = self.container_name
            && !container.is_empty()
        {
            concat_str!(self.package_name, ".", container, ".", self.symbol_name)
        } else {
            concat_str!(self.package_name, ".", self.symbol_name)
        }
    }

    pub fn target_parent(&self) -> String {
        compute_target_parent(&self.package_name, self.container_name.as_deref())
    }
}

pub fn compute_target_parent(package_name: &str, container_name: Option<&str>) -> String {
    match container_name {
        Some(container) if !container.is_empty() => {
            concat_str!(package_name, ".", container)
        }
        _ => package_name.to_string(),
    }
}

pub fn extract_auto_import_data(item: &CompletionItem) -> Option<AutoImportData> {
    let data_val = item.data.as_ref()?;
    AutoImportData::deserialize(data_val).ok()
}

pub fn is_kotlin_default_import(pkg: &str) -> bool {
    matches!(
        pkg,
        "kotlin"
            | "kotlin.annotation"
            | "kotlin.collections"
            | "kotlin.comparisons"
            | "kotlin.io"
            | "kotlin.ranges"
            | "kotlin.sequences"
            | "kotlin.text"
            | "kotlin.jvm"
            | "java.lang"
    )
}

pub fn is_import_suppressed_parts(
    file_symbols: &FileSymbols,
    package_name: &str,
    symbol_name: &str,
    container_name: Option<&str>,
) -> bool {
    if package_name.is_empty() {
        return true;
    }
    if file_symbols.package_name.as_deref() == Some(package_name) {
        return true;
    }
    if is_kotlin_default_import(package_name) {
        return true;
    }
    if file_symbols.imports.is_empty() {
        return false;
    }

    let mut target_parent = None;
    let mut full_qualified_name = None;
    for import in &file_symbols.imports {
        if import.is_star {
            let parent = target_parent
                .get_or_insert_with(|| compute_target_parent(package_name, container_name));
            let star_pkg = import.path.trim_end_matches(".*").trim();
            if star_pkg == parent {
                return true;
            }
        } else if import.alias.is_none() {
            let fqn = full_qualified_name.get_or_insert_with(|| match container_name {
                Some(container) if !container.is_empty() => {
                    concat_str!(package_name, ".", container, ".", symbol_name)
                }
                _ => concat_str!(package_name, ".", symbol_name),
            });
            if &import.path == fqn {
                return true;
            }
        }
    }
    false
}

pub fn is_import_suppressed(file_symbols: &FileSymbols, data: &AutoImportData) -> bool {
    is_import_suppressed_parts(
        file_symbols,
        &data.package_name,
        &data.symbol_name,
        data.container_name.as_deref(),
    )
}

fn check_import_header_matches(
    node: Node,
    source: &str,
    target_parent: &str,
    target_fqn: &str,
) -> bool {
    let mut cursor = node.walk();
    let mut path_str = None;
    let mut is_star = false;
    let mut has_alias = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                path_str = source
                    .get(child.start_byte()..child.end_byte())
                    .map(str::trim);
            }
            "wildcard_import" => {
                is_star = true;
            }
            "import_alias" => {
                has_alias = true;
            }
            _ => {}
        }
    }

    if let Some(path) = path_str {
        if is_star {
            path.trim_end_matches(".*").trim() == target_parent
        } else if has_alias {
            false
        } else {
            path == target_fqn
        }
    } else {
        false
    }
}

fn is_ast_import_suppressed(root: Node, source: &str, data: &AutoImportData) -> bool {
    let mut cursor = root.walk();
    let target_fqn = data.full_name();
    let target_parent = data.target_parent();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "package_header" => {
                if let Some(pkg_path) = extract_import_path(child, source)
                    && pkg_path == data.package_name
                {
                    return true;
                }
            }
            "import_list" => {
                let mut import_cursor = child.walk();
                for import_header_node in child.children(&mut import_cursor) {
                    if import_header_node.kind() == "import_header"
                        && check_import_header_matches(
                            import_header_node,
                            source,
                            &target_parent,
                            &target_fqn,
                        )
                    {
                        return true;
                    }
                }
            }
            "import_header"
                if check_import_header_matches(child, source, &target_parent, &target_fqn) =>
            {
                return true;
            }
            _ => {}
        }
    }

    false
}

pub fn compute_auto_import_edit(source: &str, data: &AutoImportData) -> Option<TextEdit> {
    if data.package_name.is_empty() || is_kotlin_default_import(&data.package_name) {
        return None;
    }

    let tree = crate::parser::parse_source(source)?;
    let root = tree.root_node();

    if is_ast_import_suppressed(root, source, data) {
        return None;
    }

    let target_fqn = data.full_name();
    let (pos, new_text) = find_ast_import_insertion(root, source, &target_fqn);
    Some(TextEdit {
        range: Range {
            start: pos,
            end: pos,
        },
        new_text,
    })
}

pub fn resolve_completion_item(
    mut item: CompletionItem,
    source: &str,
    data: &AutoImportData,
) -> CompletionItem {
    if let Some(edit) = compute_auto_import_edit(source, data) {
        let mut edits = item.additional_text_edits.take().unwrap_or_default();
        if !edits
            .iter()
            .any(|e| e.new_text == edit.new_text && e.range == edit.range)
        {
            edits.push(edit);
        }
        item.additional_text_edits = Some(edits);
    }
    item
}

pub fn resolve_auto_import_candidate<'a>(
    index: &'a crate::indexer::WorkspaceIndex,
    url: &tower_lsp::lsp_types::Url,
    decl: &crate::indexer::DeclarationItem,
    current_pkg: Option<&str>,
    current_file: Option<&crate::indexer::FileSymbols>,
) -> Option<&'a str> {
    let decl_pkg = index
        .files
        .get(url)
        .and_then(|file_symbols| file_symbols.package_name.as_deref())?;

    if Some(decl_pkg) == current_pkg || is_kotlin_default_import(decl_pkg) {
        return None;
    }

    if let Some(file_symbols) = current_file
        && is_import_suppressed_parts(
            file_symbols,
            decl_pkg,
            decl.name.as_str(),
            decl.container_name.as_deref(),
        )
    {
        return None;
    }

    Some(decl_pkg)
}
