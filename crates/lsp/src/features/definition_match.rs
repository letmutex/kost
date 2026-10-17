use tower_lsp::lsp_types::{Location, SymbolKind, Url};

use super::definition_receiver::find_explicit_import_definition;
use crate::indexer::{DeclarationItem, DeclarationRef, is_source_url};
use crate::inference::overload::{OverloadScore, score_overload_candidate_parsed};
use crate::syntax::ast_call_view::parse_call_arg;

pub fn matches_context_role(kind: SymbolKind, is_call: bool) -> bool {
    if is_call {
        matches!(kind, SymbolKind::FUNCTION | SymbolKind::CONSTRUCTOR)
    } else {
        matches!(
            kind,
            SymbolKind::PROPERTY
                | SymbolKind::VARIABLE
                | SymbolKind::ENUM_MEMBER
                | SymbolKind::FIELD
                | SymbolKind::CLASS
                | SymbolKind::OBJECT
                | SymbolKind::INTERFACE
                | SymbolKind::TYPE_PARAMETER
        )
    }
}

pub fn pick_best_declaration<'a>(
    iter: impl Iterator<Item = &'a DeclarationItem>,
    is_call: bool,
) -> Option<&'a DeclarationItem> {
    let mut fallback = None;
    for decl in iter {
        if matches_context_role(decl.kind, is_call) {
            return Some(decl);
        }
        if fallback.is_none() {
            fallback = Some(decl);
        }
    }
    fallback
}

/// Choose the best matching location, preferring source files (.kt / .kts) over bytecode (.class)
/// and matching syntactic role (function vs property/value).
pub fn pick_best_match<'a>(
    iter: impl Iterator<Item = &'a DeclarationRef>,
    is_call: bool,
) -> Option<Location> {
    let mut best_source_fallback = None;
    let mut best_bytecode_matched = None;
    let mut best_bytecode_fallback = None;

    for (url, decl) in iter {
        let is_src = is_source_url(url);
        let matches_role = matches_context_role(decl.kind, is_call);
        let loc = Location {
            uri: (**url).clone(),
            range: decl.selection_range,
        };

        if is_src && matches_role {
            return Some(loc);
        } else if is_src {
            best_source_fallback.get_or_insert(loc);
        } else if matches_role {
            best_bytecode_matched.get_or_insert(loc);
        } else {
            best_bytecode_fallback.get_or_insert(loc);
        }
    }

    best_bytecode_matched
        .or(best_source_fallback)
        .or(best_bytecode_fallback)
}

fn is_preferred_decl(
    decl: &DeclarationItem,
    score: OverloadScore,
    best: Option<(&DeclarationItem, OverloadScore)>,
    current_url: Option<&Url>,
    target_location: Option<&Location>,
) -> bool {
    let Some((best_decl, best_score)) = best else {
        return true;
    };
    score > best_score
        || (score == best_score
            && target_location.is_some_and(|target_loc| {
                current_url.is_none_or(|url| url == &target_loc.uri)
                    && decl.selection_range == target_loc.range
                    && best_decl.selection_range != target_loc.range
            }))
}

pub fn pick_best_decl_with_overload<'a>(
    iter: impl Iterator<Item = &'a DeclarationItem>,
    is_call: bool,
    raw_args: &[&str],
    inferred_types: &[Option<crate::inference::KotlinType>],
    index: &crate::indexer::WorkspaceIndex,
    current_url: Option<&Url>,
    target_location: Option<&Location>,
) -> Option<&'a DeclarationItem> {
    if !is_call {
        return pick_best_declaration(iter, is_call);
    }
    let items: Vec<&'a DeclarationItem> = iter.collect();
    let call_args: Vec<_> = raw_args.iter().map(|a| parse_call_arg(a)).collect();
    let mut best: Option<(&'a DeclarationItem, OverloadScore)> = None;
    for decl in &items {
        if let Some(score) =
            score_overload_candidate_parsed(decl, true, &call_args, inferred_types, index)
            && is_preferred_decl(
                decl,
                score,
                best.as_ref().copied(),
                current_url,
                target_location,
            )
        {
            best = Some((decl, score));
        }
    }
    if raw_args.is_empty() {
        best.map(|(decl, _)| decl)
            .or_else(|| pick_best_declaration(items.into_iter(), is_call))
    } else {
        best.map(|(decl, _)| decl)
    }
}

fn is_preferred_match(
    cand: &DeclarationRef,
    score: OverloadScore,
    best: Option<(&DeclarationRef, OverloadScore)>,
    target_location: Option<&Location>,
) -> bool {
    let Some((best_cand, best_score)) = best else {
        return true;
    };
    score > best_score
        || (score == best_score
            && target_location.is_some_and(|target_loc| {
                cand.0.as_ref() == &target_loc.uri
                    && cand.1.selection_range == target_loc.range
                    && !(best_cand.0.as_ref() == &target_loc.uri
                        && best_cand.1.selection_range == target_loc.range)
            }))
}

pub fn pick_best_match_with_overload<'a>(
    iter: impl Iterator<Item = &'a DeclarationRef>,
    is_call: bool,
    raw_args: &[&str],
    inferred_types: &[Option<crate::inference::KotlinType>],
    index: &crate::indexer::WorkspaceIndex,
    target_location: Option<&Location>,
) -> Option<Location> {
    if !is_call {
        return pick_best_match(iter, is_call);
    }
    let items: Vec<&'a DeclarationRef> = iter.collect();
    let call_args: Vec<_> = raw_args.iter().map(|a| parse_call_arg(a)).collect();
    if raw_args.is_empty() && items.iter().any(|c| is_source_url(&c.0)) {
        let mut best_src: Option<(&'a DeclarationRef, OverloadScore)> = None;
        for cand in items.iter().filter(|c| is_source_url(&c.0)) {
            if let Some(score) = score_overload_candidate_parsed(
                cand.1.as_ref(),
                true,
                &call_args,
                inferred_types,
                index,
            ) && is_preferred_match(cand, score, best_src.as_ref().copied(), target_location)
            {
                best_src = Some((cand, score));
            }
        }
        if let Some((best_ref, _)) = best_src {
            return Some(Location {
                uri: (*best_ref.0).clone(),
                range: best_ref.1.selection_range,
            });
        }
    }
    let mut best: Option<(&'a DeclarationRef, OverloadScore)> = None;
    for cand in &items {
        let is_src = is_source_url(&cand.0);
        if let Some(score) = score_overload_candidate_parsed(
            cand.1.as_ref(),
            is_src,
            &call_args,
            inferred_types,
            index,
        ) && is_preferred_match(cand, score, best.as_ref().copied(), target_location)
        {
            best = Some((cand, score));
        }
    }
    if let Some((best_ref, _)) = best {
        return Some(Location {
            uri: (*best_ref.0).clone(),
            range: best_ref.1.selection_range,
        });
    }
    pick_best_match(items.into_iter(), is_call)
}

fn find_star_import_definition<F>(
    current_file: &crate::indexer::FileSymbols,
    decls: &[crate::indexer::DeclarationRef],
    is_call: bool,
    raw_args: &[&str],
    inferred_types: &[Option<crate::inference::KotlinType>],
    index: &crate::indexer::WorkspaceIndex,
    is_visible: &F,
    target_location: Option<&Location>,
) -> Option<Location>
where
    F: Fn(&DeclarationItem) -> bool,
{
    let has_star_import = current_file.imports.iter().any(|import| import.is_star);
    if !has_star_import {
        return None;
    }
    pick_best_match_with_overload(
        decls.iter().filter(|(url, decl)| {
            !decl.is_private
                && is_visible(decl)
                && index
                    .files
                    .get(url.as_ref())
                    .and_then(|file_syms| file_syms.package_name.as_deref())
                    .is_some_and(|pkg| {
                        current_file
                            .imports
                            .iter()
                            .any(|import| import.is_star && import.path == pkg)
                    })
        }),
        is_call,
        raw_args,
        inferred_types,
        index,
        target_location,
    )
}

pub fn find_global_or_imported_definition<F>(
    index: &crate::indexer::WorkspaceIndex,
    current_file: Option<&crate::indexer::FileSymbols>,
    symbol_name: &str,
    is_call: bool,
    raw_args: &[&str],
    inferred_types: &[Option<crate::inference::KotlinType>],
    is_visible: &F,
    target_location: Option<&Location>,
) -> Option<Location>
where
    F: Fn(&DeclarationItem) -> bool,
{
    if let Some(loc) = find_explicit_import_definition(
        index,
        current_file,
        symbol_name,
        is_call,
        raw_args,
        inferred_types,
        target_location,
    ) {
        return Some(loc);
    }

    let decls = index.name_to_decls.get(symbol_name)?;
    let current_pkg = current_file.and_then(|file_syms| file_syms.package_name.as_deref());

    if let Some(pkg) = current_pkg
        && let Some(loc) = pick_best_match_with_overload(
            decls.iter().filter(|(url, decl)| {
                !decl.is_private
                    && is_visible(decl)
                    && index
                        .files
                        .get(url.as_ref())
                        .and_then(|file_syms| file_syms.package_name.as_deref())
                        == Some(pkg)
            }),
            is_call,
            raw_args,
            inferred_types,
            index,
            target_location,
        )
    {
        return Some(loc);
    }

    if let Some(file_syms) = current_file
        && let Some(loc) = find_star_import_definition(
            file_syms,
            decls,
            is_call,
            raw_args,
            inferred_types,
            index,
            is_visible,
            target_location,
        )
    {
        return Some(loc);
    }

    pick_best_match_with_overload(
        decls
            .iter()
            .filter(|(_, decl)| !decl.is_private && is_visible(decl)),
        is_call,
        raw_args,
        inferred_types,
        index,
        target_location,
    )
}
