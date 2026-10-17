use smol_str::SmolStr;
use tower_lsp::lsp_types::Url;

use super::target::ReferenceTarget;
use crate::indexer::{ImportItem, WorkspaceIndex};
use crate::inference::qualification::resolve::DEFAULT_KOTLIN_PACKAGES;

#[inline]
pub fn is_identifier_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

#[inline]
fn is_ascii_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[inline]
fn is_token_boundary_valid(source: &str, abs_pos: usize, end_pos: usize) -> bool {
    let source_bytes = source.as_bytes();
    let before_ok = abs_pos == 0 || {
        let prev_byte = source_bytes[abs_pos - 1];
        if prev_byte < 128 {
            !is_ascii_ident_byte(prev_byte)
        } else {
            source[..abs_pos]
                .chars()
                .next_back()
                .is_none_or(|character| !is_identifier_char(character))
        }
    };
    if !before_ok {
        return false;
    }

    end_pos >= source_bytes.len() || {
        let next_byte = source_bytes[end_pos];
        if next_byte < 128 {
            !is_ascii_ident_byte(next_byte)
        } else {
            source[end_pos..]
                .chars()
                .next()
                .is_none_or(|character| !is_identifier_char(character))
        }
    }
}

pub fn contains_identifier_token(source: &str, target_name: &str) -> bool {
    let target_len = target_name.len();
    if target_len == 0 || source.len() < target_len {
        return false;
    }

    let mut start = 0;
    while let Some(pos) = source[start..].find(target_name) {
        let abs_pos = start + pos;
        let end_pos = abs_pos + target_len;
        if is_token_boundary_valid(source, abs_pos, end_pos) {
            return true;
        }

        let next_start = source[abs_pos..]
            .char_indices()
            .nth(1)
            .map(|(idx, _)| abs_pos + idx)
            .unwrap_or(source.len());
        start = next_start;
    }

    false
}

pub(crate) fn import_matches_target(imp: &ImportItem, target_name: &str, pkg: &str) -> bool {
    if imp.is_star {
        return imp.path == pkg;
    }
    if imp.alias.as_deref() == Some(target_name) {
        return imp.path.starts_with(pkg);
    }
    if imp.path.ends_with(target_name) {
        let prefix_len = imp.path.len() - target_name.len();
        if prefix_len > 0 && imp.path.as_bytes()[prefix_len - 1] == b'.' {
            let imp_pkg = &imp.path[..prefix_len - 1];
            return imp_pkg == pkg;
        }
        return prefix_len == 0 && pkg.is_empty();
    }
    false
}

pub fn is_file_candidate(
    index: &WorkspaceIndex,
    cand_url: &Url,
    target: &ReferenceTarget,
    target_pkg: Option<&str>,
    is_top_level: bool,
    source: &str,
) -> bool {
    let clean_container = target
        .container_name
        .as_deref()
        .map(|container| container.strip_suffix(".Companion").unwrap_or(container));
    let simple_container = clean_container.map(|clean| clean.rsplit('.').next().unwrap_or(clean));
    let producers = simple_container
        .map(|simple| find_producer_names(index, simple))
        .unwrap_or_default();
    is_file_candidate_with_producers(
        index,
        cand_url,
        target,
        target_pkg,
        is_top_level,
        source,
        &producers,
    )
}

pub fn is_file_candidate_with_producers(
    index: &WorkspaceIndex,
    cand_url: &Url,
    target: &ReferenceTarget,
    target_pkg: Option<&str>,
    is_top_level: bool,
    source: &str,
    producer_names: &[SmolStr],
) -> bool {
    if !contains_identifier_token(source, &target.name) {
        return false;
    }

    if cand_url == &target.location.uri {
        return true;
    }

    let Some(pkg) = target_pkg else {
        return true;
    };
    if DEFAULT_KOTLIN_PACKAGES.contains(&pkg) {
        return true;
    }

    let Some(file_syms) = index.files.get(cand_url) else {
        return true;
    };

    if is_top_level {
        let in_same_pkg = file_syms.package_name.as_deref() == Some(pkg);
        let has_import = file_syms
            .imports
            .iter()
            .any(|import_item| import_matches_target(import_item, &target.name, pkg));
        return in_same_pkg || has_import || source.contains(pkg);
    }

    if let Some(ref container) = target.container_name {
        return is_container_candidate(
            index,
            file_syms,
            target,
            container,
            pkg,
            source,
            producer_names,
        );
    }

    true
}

fn is_container_candidate(
    index: &WorkspaceIndex,
    file_syms: &crate::indexer::FileSymbols,
    target: &ReferenceTarget,
    container: &str,
    pkg: &str,
    source: &str,
    producer_names: &[SmolStr],
) -> bool {
    if target.is_extension {
        let in_same_pkg = file_syms.package_name.as_deref() == Some(pkg);
        let has_import = file_syms
            .imports
            .iter()
            .any(|import_item| import_matches_target(import_item, &target.name, pkg));
        return in_same_pkg || has_import;
    }

    let clean_container = container.strip_suffix(".Companion").unwrap_or(container);
    let simple_container = clean_container
        .rsplit('.')
        .next()
        .unwrap_or(clean_container);
    if contains_identifier_token(source, simple_container) {
        return true;
    }

    if has_subtype_token(index, simple_container, source) {
        return true;
    }

    if target.is_override && has_supertype_token(index, simple_container, source) {
        return true;
    }

    if has_type_alias_token(index, simple_container, source) {
        return true;
    }

    has_producer_token(source, producer_names)
}

fn has_supertype_token(index: &WorkspaceIndex, container_name: &str, source: &str) -> bool {
    for supertype in index.get_all_supertypes(container_name) {
        let clean = supertype
            .split(['<', '?'])
            .next()
            .unwrap_or(&supertype)
            .trim();
        let simple = clean.rsplit('.').next().unwrap_or(clean);
        if contains_identifier_token(source, simple) {
            return true;
        }
    }
    false
}

fn has_subtype_token(index: &WorkspaceIndex, container_name: &str, source: &str) -> bool {
    for (subclass_name, supertypes) in &index.class_supertypes {
        let matches_container = supertypes.iter().any(|supertype_name| {
            let clean_super = supertype_name
                .split('<')
                .next()
                .unwrap_or(supertype_name)
                .trim();
            clean_super == container_name
                || clean_super
                    .strip_suffix(container_name)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        });
        if matches_container {
            let simple_subclass = subclass_name.rsplit('.').next().unwrap_or(subclass_name);
            if contains_identifier_token(source, simple_subclass) {
                return true;
            }
        }
    }
    false
}

fn has_type_alias_token(index: &WorkspaceIndex, container_name: &str, source: &str) -> bool {
    for (alias_name, target_type) in &index.type_aliases {
        if contains_identifier_token(target_type, container_name)
            && contains_identifier_token(source, alias_name)
        {
            return true;
        }
    }
    false
}

fn decl_produces_container(decl: &crate::indexer::DeclarationItem, container_name: &str) -> bool {
    if decl.is_private {
        return false;
    }
    let Some(ref sig) = decl.signature else {
        return false;
    };
    crate::syntax::signature::extract_return_type_from_signature(sig)
        .is_some_and(|ret| contains_identifier_token(&ret, container_name))
}

pub fn find_producer_names(index: &WorkspaceIndex, container_name: &str) -> Vec<SmolStr> {
    let mut producers = Vec::new();
    for (name, decl_refs) in &index.name_to_decls {
        let produces = decl_refs
            .iter()
            .any(|(_, decl)| decl_produces_container(decl, container_name));
        if produces {
            producers.push(name.clone());
        }
    }
    producers
}

fn has_producer_token(source: &str, producer_names: &[SmolStr]) -> bool {
    for producer in producer_names {
        if contains_identifier_token(source, producer.as_str()) {
            return true;
        }
    }
    false
}
