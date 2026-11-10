use tower_lsp::lsp_types::SymbolKind;
use url::Url;

use crate::indexer::WorkspaceIndex;
use crate::syntax::symbol_item::{DeclarationItem, ImportItem};

/// Check if a declaration in `decl_url` is visible and accessible from `current_url`
pub fn is_declaration_visible(
    index: &WorkspaceIndex,
    current_url: &Url,
    decl_url: &Url,
    decl: &DeclarationItem,
) -> bool {
    if decl_url == current_url {
        return true;
    }
    if decl.is_private {
        return false;
    }

    let current_file = index.files.get(current_url);
    let decl_file = index.files.get(decl_url);
    let current_pkg = current_file.and_then(|f| f.package_name.as_deref());
    let decl_pkg = decl_file
        .and_then(|f| f.package_name.as_deref())
        .or_else(|| {
            decl.container_name
                .as_deref()
                .and_then(|c| c.rsplit_once('.').map(|(p, _)| p))
        });

    if let (Some(cp), Some(dp)) = (current_pkg, decl_pkg) {
        if cp == dp {
            return true;
        }
    } else if current_pkg.is_none()
        && decl_pkg.is_none()
        && crate::indexer::is_source_url(decl_url)
        && crate::indexer::is_source_url(current_url)
    {
        return true;
    }

    if decl_pkg.is_some_and(|p| p.starts_with("kotlin.") || p == "kotlin" || p == "java.lang")
        || decl_url.as_str().contains("kotlin-stdlib")
    {
        return true;
    }

    if let Some(file) = current_file {
        for import in &file.imports {
            if matches_import(import, decl_pkg, decl) {
                return true;
            }
        }
    }

    false
}

pub fn is_aliased_import_target(
    index: &WorkspaceIndex,
    import: &ImportItem,
    decl_url: &Url,
    decl: &DeclarationItem,
) -> bool {
    if decl.is_private {
        return false;
    }
    let decl_pkg = index
        .files
        .get(decl_url)
        .and_then(|f| f.package_name.as_deref())
        .or_else(|| {
            decl.container_name
                .as_deref()
                .and_then(|c| c.rsplit_once('.').map(|(p, _)| p))
        });
    matches_import_path(import, decl_pkg, decl)
}

fn matches_import(import: &ImportItem, decl_pkg: Option<&str>, decl: &DeclarationItem) -> bool {
    if import.alias.is_some() && import.alias.as_deref() != Some(decl.name.as_str()) {
        return false;
    }
    matches_import_path(import, decl_pkg, decl)
}

fn matches_import_path(
    import: &ImportItem,
    decl_pkg: Option<&str>,
    decl: &DeclarationItem,
) -> bool {
    let is_nested_type = decl.container_name.is_some()
        && matches!(
            decl.kind,
            SymbolKind::CLASS | SymbolKind::INTERFACE | SymbolKind::ENUM | SymbolKind::OBJECT
        );
    if import.is_star {
        if let Some(dp) = decl_pkg {
            let star_path = import.path.trim_end_matches(".*").trim();
            if !is_nested_type && star_path == dp {
                return true;
            }
            if let Some(ref c) = decl.container_name {
                let target_container_path = common::concat_str!(dp, ".", c);
                if star_path == target_container_path {
                    return true;
                }
            }
        }
    } else if let Some(dp) = decl_pkg {
        if !is_nested_type && import.path == common::concat_str!(dp, ".", decl.name.as_str()) {
            return true;
        }
        if let Some(ref c) = decl.container_name {
            let target_path = common::concat_str!(dp, ".", c, ".", decl.name.as_str());
            if import.path == target_path {
                return true;
            }
        }
    } else if import.path == decl.name.as_str()
        || (import.path.ends_with(decl.name.as_str())
            && import
                .path
                .as_bytes()
                .get(import.path.len() - decl.name.len() - 1)
                == Some(&b'.'))
    {
        return true;
    }
    false
}

pub fn is_container_member_imported(
    index: &WorkspaceIndex,
    current_url: &Url,
    short_container: &str,
    member_name: &str,
) -> bool {
    let Some(file) = index.files.get(current_url) else {
        return false;
    };
    file.imports.iter().any(|imp| {
        let path = imp.path.trim_end_matches(".*");
        if imp.is_star {
            path == short_container || path.ends_with(&common::concat_str!(".", short_container))
        } else {
            path.ends_with(&common::concat_str!(short_container, ".", member_name))
        }
    })
}
