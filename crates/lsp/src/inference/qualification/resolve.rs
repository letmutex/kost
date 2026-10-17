use tower_lsp::lsp_types::SymbolKind;
use url::Url;

use crate::indexer::WorkspaceIndex;
use crate::inference::model::PrimitiveType;
use crate::inference::typealias::resolve_type_alias_or_import;
use crate::syntax::symbol_item::{DeclarationItem, FileSymbols, ImportItem};

fn find_matching_explicit_import(file: &FileSymbols, clean: &str) -> Option<String> {
    for import in &file.imports {
        let is_alias = import.alias.as_deref() == Some(clean);
        let is_explicit = !import.is_star
            && (import.path.ends_with(clean)
                && (import.path.len() == clean.len()
                    || import.path.as_bytes()[import.path.len() - clean.len() - 1] == b'.'));
        if is_alias || is_explicit {
            return Some(import.path.clone());
        }
    }
    None
}

fn format_qualified_decl(
    pkg: Option<&str>,
    container: Option<&smol_str::SmolStr>,
    clean: &str,
) -> String {
    let container_prefix = container
        .map(|c| crate::concat_str!(c, "."))
        .unwrap_or_default();
    if let Some(p) = pkg {
        crate::concat_str!(p, ".", container_prefix, clean)
    } else {
        crate::concat_str!(container_prefix, clean)
    }
}

pub const DEFAULT_KOTLIN_PACKAGES: &[&str] = &[
    "kotlin",
    "kotlin.annotation",
    "kotlin.collections",
    "kotlin.comparisons",
    "kotlin.io",
    "kotlin.ranges",
    "kotlin.sequences",
    "kotlin.text",
    "java.lang",
    "kotlin.jvm",
];

pub fn resolve_type_by_imports_or_package(
    index: &WorkspaceIndex,
    current_url: &Url,
    clean: &str,
    is_visible: &impl Fn(&Url, &DeclarationItem) -> bool,
) -> Option<String> {
    let current_file = index.files.get(current_url);
    if let Some(file) = current_file
        && let Some(imported) = find_matching_explicit_import(file, clean)
    {
        return Some(imported);
    }

    let decls = index.name_to_decls.get(clean)?;
    let is_type = |d: &DeclarationItem| {
        matches!(
            d.kind,
            SymbolKind::CLASS | SymbolKind::OBJECT | SymbolKind::INTERFACE | SymbolKind::ENUM
        )
    };

    let current_pkg = current_file.and_then(|f| f.package_name.as_deref());
    if let Some((_, decl)) = decls.iter().find(|(u, d)| {
        is_visible(u, d)
            && is_type(d)
            && index
                .files
                .get(u.as_ref())
                .and_then(|f| f.package_name.as_deref())
                == current_pkg
    }) {
        return Some(format_qualified_decl(
            current_pkg,
            decl.container_name.as_ref(),
            clean,
        ));
    }

    if let Some(file) = current_file {
        for import in &file.imports {
            if import.is_star
                && let Some((_, decl)) = decls.iter().find(|(u, d)| {
                    !d.is_private
                        && is_type(d)
                        && index
                            .files
                            .get(u.as_ref())
                            .and_then(|f| f.package_name.as_deref())
                            == Some(&import.path)
                })
            {
                return Some(format_qualified_decl(
                    Some(&import.path),
                    decl.container_name.as_ref(),
                    clean,
                ));
            }
        }
    }

    for &default_pkg in DEFAULT_KOTLIN_PACKAGES {
        if let Some((_, decl)) = decls.iter().find(|(u, d)| {
            is_visible(u, d)
                && is_type(d)
                && index
                    .files
                    .get(u.as_ref())
                    .and_then(|f| f.package_name.as_deref())
                    == Some(default_pkg)
        }) {
            return Some(format_qualified_decl(
                Some(default_pkg),
                decl.container_name.as_ref(),
                clean,
            ));
        }
    }

    None
}

pub fn resolve_full_type_name(
    index: &WorkspaceIndex,
    current_url: &Url,
    type_name: &str,
) -> String {
    let clean = type_name.trim();
    if clean.is_empty() || clean == "?" || PrimitiveType::from_name(clean).is_some() {
        return clean.to_string();
    }
    let is_nullable = clean.ends_with('?');
    let unnull = clean.trim_end_matches('?').trim();
    let (base, generic_suffix) = match unnull.split_once('<') {
        Some((b, g)) => (b.trim(), Some(crate::concat_str!("<", g))),
        None => (unnull, None),
    };

    let aliased = resolve_type_alias_or_import(index, current_url, base);
    let target = if aliased.is_empty() { base } else { &aliased };

    if PrimitiveType::from_name(target).is_some() {
        let with_gen = match generic_suffix {
            Some(g) => crate::concat_str!(target, g),
            None => target.to_string(),
        };
        return if is_nullable && !with_gen.ends_with('?') {
            crate::concat_str!(with_gen, "?")
        } else {
            with_gen
        };
    }

    let is_visible = |url: &Url, decl: &DeclarationItem| !decl.is_private || url == current_url;
    let resolved_base = if let Some((first, rest)) = target.split_once('.') {
        if let Some(resolved_first) =
            resolve_type_by_imports_or_package(index, current_url, first, &is_visible)
        {
            crate::concat_str!(resolved_first.as_str(), ".", rest)
        } else {
            target.to_string()
        }
    } else {
        resolve_type_by_imports_or_package(index, current_url, target, &is_visible)
            .unwrap_or_else(|| target.to_string())
    };

    let with_gen = match generic_suffix {
        Some(g) => crate::concat_str!(resolved_base, g),
        None => resolved_base,
    };
    if is_nullable && !with_gen.ends_with('?') {
        crate::concat_str!(with_gen, "?")
    } else {
        with_gen
    }
}

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

fn matches_import(import: &ImportItem, decl_pkg: Option<&str>, decl: &DeclarationItem) -> bool {
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
                let target_container_path = crate::concat_str!(dp, ".", c);
                if star_path == target_container_path {
                    return true;
                }
            }
        }
    } else if let Some(dp) = decl_pkg {
        if !is_nested_type && import.path == crate::concat_str!(dp, ".", decl.name.as_str()) {
            return true;
        }
        if let Some(ref c) = decl.container_name {
            let target_path = crate::concat_str!(dp, ".", c, ".", decl.name.as_str());
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

pub fn match_qualified_class(
    index: &WorkspaceIndex,
    clean: &str,
    tracer: &mut crate::inference::trace::Tracer,
) -> Option<crate::inference::types::KotlinType> {
    let (pkg, class_name) = clean.rsplit_once('.')?;
    let decls = index.name_to_decls.get(class_name)?;
    let is_match = decls.iter().any(|(u, d)| {
        !d.is_private
            && matches!(
                d.kind,
                SymbolKind::CLASS | SymbolKind::OBJECT | SymbolKind::INTERFACE | SymbolKind::ENUM
            )
            && index
                .files
                .get(u.as_ref())
                .and_then(|f| f.package_name.as_deref())
                == Some(pkg)
    });
    if is_match {
        tracer.record_step(|| crate::concat_str!("Matched qualified class `", clean, "`"));
        crate::inference::types::parse_type_signature(clean)
    } else {
        None
    }
}
